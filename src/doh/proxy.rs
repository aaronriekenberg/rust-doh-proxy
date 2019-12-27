use crate::doh::cache::{get_cache_key, Cache, CacheObject};
use crate::doh::client::DOHClient;
use crate::doh::config::Configuration;

use log::{debug, info, warn};

use std::convert::TryFrom;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

use trust_dns_proto::error::ProtoResult;
use trust_dns_proto::op::Message;
use trust_dns_proto::rr::resource::Record;
use trust_dns_proto::serialize::binary::{BinDecodable, BinDecoder, BinEncodable, BinEncoder};

pub struct DOHProxy {
    configuration: Configuration,
    cache: Cache,
    doh_client: DOHClient,
}

impl DOHProxy {
    pub fn new(configuration: Configuration) -> Arc<Self> {
        let cache_configuration = configuration.cache_configuration().clone();
        let client_configuration = configuration.client_configuration().clone();

        Arc::new(DOHProxy {
            configuration,
            cache: Cache::new(cache_configuration),
            doh_client: DOHClient::new(client_configuration),
        })
    }

    fn encode_dns_message(&self, message: &Message) -> ProtoResult<Vec<u8>> {
        let mut request_buffer = Vec::new();

        let mut encoder = BinEncoder::new(&mut request_buffer);
        match message.emit(&mut encoder) {
            Ok(()) => {
                debug!(
                    "encoded message request_buffer.len = {}",
                    request_buffer.len()
                );
                Ok(request_buffer)
            }
            Err(e) => {
                warn!("error encoding message request buffer {}", e);
                Err(e)
            }
        }
    }

    fn decode_dns_message_vec(&self, buffer: Vec<u8>) -> ProtoResult<Message> {
        let mut decoder = BinDecoder::new(&buffer);
        match Message::read(&mut decoder) {
            Ok(message) => Ok(message),
            Err(e) => {
                warn!("error decoding dns message {}", e);
                Err(e)
            }
        }
    }

    fn decode_dns_message_slice(&self, buffer: &[u8]) -> ProtoResult<Message> {
        let mut decoder = BinDecoder::new(&buffer);
        match Message::read(&mut decoder) {
            Ok(message) => Ok(message),
            Err(e) => {
                warn!("error decoding dns message {}", e);
                Err(e)
            }
        }
    }

    fn build_failure_response_message(&self, request: &Message) -> Message {
        let mut response_message = request.clone();
        response_message.set_message_type(trust_dns_proto::op::MessageType::Response);
        response_message.set_response_code(trust_dns_proto::op::ResponseCode::ServFail);
        response_message
    }

    fn build_failure_response_buffer(&self, request: &Message) -> Option<Vec<u8>> {
        match self.encode_dns_message(&self.build_failure_response_message(request)) {
            Err(e) => {
                warn!("build_failure_response_buffer encode error {}", e);
                None
            }
            Ok(buffer) => Some(buffer),
        }
    }

    async fn make_doh_request(&self, doh_request_message: Message) -> Option<Message> {
        let request_buffer = match self.encode_dns_message(&doh_request_message) {
            Err(e) => {
                warn!("encode_dns_message error {}", e);
                return None;
            }
            Ok(buffer) => buffer,
        };

        let doh_response = match self.doh_client.make_doh_request(request_buffer).await {
            Err(e) => {
                warn!("make_doh_request error {}", e);
                return None;
            }
            Ok(doh_response) => doh_response,
        };

        let response_buffer = match doh_response {
            crate::doh::client::DOHResponse::HTTPRequestError => {
                warn!("got http request error");
                return None;
            }
            crate::doh::client::DOHResponse::HTTPRequestSuccess(response_buffer) => response_buffer,
        };

        debug!("got response_buffer length = {}", response_buffer.len());

        let response_message = match self.decode_dns_message_vec(response_buffer) {
            Err(e) => {
                warn!("decode_dns_message error {}", e);
                return None;
            }
            Ok(message) => message,
        };

        Some(response_message)
    }

    fn clamp_and_get_min_ttl_seconds(&self, response_message: &mut Message) -> u32 {
        let clamp_min_ttl_seconds = self
            .configuration
            .proxy_configuration()
            .clamp_min_ttl_seconds();
        let clamp_max_ttl_seconds = self
            .configuration
            .proxy_configuration()
            .clamp_max_ttl_seconds();

        let mut found_record_ttl = false;
        let mut record_min_ttl_seconds: u32 = clamp_min_ttl_seconds;

        let mut process_record = |record: &mut Record| {
            let mut ttl = record.ttl();

            ttl = std::cmp::max(ttl, clamp_min_ttl_seconds);
            ttl = std::cmp::min(ttl, clamp_max_ttl_seconds);

            if (!found_record_ttl) || (ttl < record_min_ttl_seconds) {
                record_min_ttl_seconds = ttl;
                found_record_ttl = true;
            }
            record.set_ttl(ttl);
        };

        for mut record in response_message.take_answers() {
            process_record(&mut record);
            response_message.add_answer(record);
        }
        for mut record in response_message.take_name_servers() {
            process_record(&mut record);
            response_message.add_name_server(record);
        }
        for mut record in response_message.take_additionals() {
            process_record(&mut record);
            response_message.add_additional(record);
        }

        record_min_ttl_seconds
    }

    async fn clamp_ttl_and_cache_response(
        &self,
        cache_key: String,
        mut response_message: Message,
    ) -> Message {
        if !((response_message.response_code() == trust_dns_proto::op::ResponseCode::NoError)
            || (response_message.response_code() == trust_dns_proto::op::ResponseCode::NXDomain))
        {
            return response_message;
        }

        let min_ttl_seconds = self.clamp_and_get_min_ttl_seconds(&mut response_message);

        if min_ttl_seconds == 0 {
            return response_message;
        }

        if cache_key.is_empty() {
            return response_message;
        }

        let now = Instant::now();
        let min_ttl_duration = Duration::from_secs(min_ttl_seconds.into());
        let expiration_time = now + min_ttl_duration;

        self.cache
            .put(
                cache_key,
                CacheObject::new(response_message.clone(), now, expiration_time),
            )
            .await;

        response_message
    }

    async fn get_message_for_cache_hit(
        &self,
        cache_key: &String,
        request_id: u16,
    ) -> Option<Message> {
        let mut cache_object = match self.cache.get(&cache_key).await {
            None => return None,
            Some(cache_object) => cache_object,
        };

        if cache_object.expired(Instant::now()) {
            return None;
        }

        let seconds_to_subtract_from_ttl = cache_object.duration_in_cache().as_secs();
        let mut ok = true;

        let mut adjust_record_ttl = |record: &mut Record| {
            let original_ttl = u64::from(record.ttl());
            if seconds_to_subtract_from_ttl > original_ttl {
                ok = false;
            } else {
                let new_ttl = original_ttl - seconds_to_subtract_from_ttl;
                let new_ttl = match u32::try_from(new_ttl) {
                    Ok(new_ttl) => new_ttl,
                    Err(e) => {
                        warn!(
                            "get_message_for_cache_hit new_ttl overflow {} {}",
                            new_ttl, e
                        );
                        ok = false;
                        0
                    }
                };
                record.set_ttl(new_ttl);
            }
        };

        let response_message = cache_object.message_mut();

        for mut record in response_message.take_answers() {
            adjust_record_ttl(&mut record);
            response_message.add_answer(record);
        }
        for mut record in response_message.take_name_servers() {
            adjust_record_ttl(&mut record);
            response_message.add_name_server(record);
        }
        for mut record in response_message.take_additionals() {
            adjust_record_ttl(&mut record);
            response_message.add_additional(record);
        }

        if !ok {
            return None;
        }

        response_message.set_id(request_id);

        Some(cache_object.message())
    }

    async fn process_request_message(&self, request_message: &Message) -> Message {
        debug!(
            "process_request_message request_message {:#?}",
            request_message
        );

        if request_message.queries().is_empty() {
            warn!("request_message.queries is empty");
            return self.build_failure_response_message(&request_message);
        }

        let cache_key = get_cache_key(&request_message);

        if let Some(response_message) = self
            .get_message_for_cache_hit(&cache_key, request_message.header().id())
            .await
        {
            return response_message;
        }

        let mut doh_request_message = request_message.clone();
        doh_request_message.set_id(0);

        let response_message = match self.make_doh_request(doh_request_message).await {
            None => return self.build_failure_response_message(&request_message),
            Some(response_message) => response_message,
        };

        let mut response_message = self
            .clamp_ttl_and_cache_response(cache_key, response_message)
            .await;
        response_message.set_id(request_message.header().id());

        response_message
    }

    pub(in crate::doh) async fn process_request_packet_buffer(
        &self,
        request_buffer: &[u8],
    ) -> Option<Vec<u8>> {
        debug!(
            "process_request_packet_buffer received {}",
            request_buffer.len()
        );

        let request_message = match self.decode_dns_message_slice(&request_buffer) {
            Err(e) => {
                warn!("decode_dns_message request error {}", e);
                return None;
            }
            Ok(message) => message,
        };

        let response_message = self.process_request_message(&request_message).await;

        match self.encode_dns_message(&response_message) {
            Err(e) => {
                warn!("encode_dns_message response error {}", e);
                self.build_failure_response_buffer(&request_message)
            }
            Ok(buffer) => Some(buffer),
        }
    }

    async fn run_periodic_timer(self: Arc<Self>) {
        info!("begin run_periodic_timer");

        loop {
            tokio::time::delay_for(Duration::from_secs(
                self.configuration.timer_interval_seconds(),
            ))
            .await;

            let cache_items_purged = self.cache.periodic_purge().await;
            info!(
                "run_periodic_timer pop cache len={} cache_items_purged={}",
                self.cache.len().await,
                cache_items_purged,
            );
        }
    }

    pub async fn run(self: Arc<Self>) -> Result<(), Box<dyn Error>> {
        info!("begin run");

        tokio::spawn(Arc::clone(&self).run_periodic_timer());

        let tcp_server = crate::doh::tcpserver::TCPServer::new(
            self.configuration.server_configuration().clone(),
            Arc::clone(&self),
        );
        tokio::spawn(async move {
            if let Err(e) = tcp_server.run().await {
                warn!("run_tcp_server returned error {}", e);
            }
        });

        let udp_server = crate::doh::udpserver::UDPServer::new(
            self.configuration.server_configuration().clone(),
            Arc::clone(&self),
        );
        udp_server.run().await
    }
}
