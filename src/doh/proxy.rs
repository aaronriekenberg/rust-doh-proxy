use std::convert::TryFrom;
use std::error::Error;
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{debug, info, warn};
use trust_dns_proto::op::Message;
use trust_dns_proto::rr::resource::Record;

use crate::doh::cache::{Cache, CacheObject};
use crate::doh::client::DOHClient;
use crate::doh::config::Configuration;
use crate::doh::localdomain::LocalDomainCache;
use crate::doh::metrics::Metrics;
use crate::doh::request_key::RequestKey;
use crate::doh::utils;

pub struct DOHProxy {
    configuration: Configuration,
    local_domain_cache: LocalDomainCache,
    cache: Cache,
    doh_client: DOHClient,
    metrics: Arc<Metrics>,
}

impl DOHProxy {
    pub fn new(configuration: Configuration) -> Result<Arc<Self>, Box<dyn Error>> {
        let forward_domain_configurations = configuration.forward_domain_configurations().clone();
        let reverse_domain_configurations = configuration.reverse_domain_configurations().clone();
        let cache_configuration = configuration.cache_configuration().clone();
        let client_configuration = configuration.client_configuration().clone();

        Ok(Arc::new(DOHProxy {
            configuration,
            local_domain_cache: LocalDomainCache::new(
                forward_domain_configurations,
                reverse_domain_configurations,
            )?,
            cache: Cache::new(cache_configuration),
            doh_client: DOHClient::new(client_configuration)?,
            metrics: Metrics::new(),
        }))
    }

    fn build_failure_response_message(&self, request: &Message) -> Message {
        let mut response_message = request.clone();
        response_message.set_message_type(trust_dns_proto::op::MessageType::Response);
        response_message.set_response_code(trust_dns_proto::op::ResponseCode::ServFail);
        response_message
    }

    fn build_failure_response_buffer(&self, request: &Message) -> Option<Vec<u8>> {
        match utils::encode_dns_message(self.build_failure_response_message(request)) {
            Err(e) => {
                warn!("build_failure_response_buffer encode error {}", e);
                None
            }
            Ok(buffer) => Some(buffer),
        }
    }

    async fn make_doh_request(&self, request_message: &Message) -> Option<Message> {
        let mut doh_request_message = request_message.clone();
        doh_request_message.set_id(0);
        let request_buffer = match utils::encode_dns_message(doh_request_message) {
            Err(e) => {
                warn!("encode_dns_message error {}", e);
                return None;
            }
            Ok(buffer) => buffer,
        };

        let response_buffer = match self.doh_client.make_doh_request(request_buffer).await {
            Err(e) => {
                warn!("make_doh_request error {}", e);
                self.metrics.doh_request_errors().increment_value();
                return None;
            }
            Ok(response_buffer) => response_buffer,
        };

        debug!("got response_buffer length = {}", response_buffer.len());

        let response_message = match utils::decode_dns_message(response_buffer) {
            Err(e) => {
                warn!("decode_dns_message error {}", e);
                return None;
            }
            Ok(message) => message,
        };

        Some(response_message)
    }

    fn clamp_and_get_min_ttl_duration(&self, response_message: Message) -> (Duration, Message) {
        let clamp_min_ttl_seconds = self
            .configuration
            .proxy_configuration()
            .clamp_min_ttl_seconds();
        let clamp_max_ttl_seconds = self
            .configuration
            .proxy_configuration()
            .clamp_max_ttl_seconds();

        let mut found_record_ttl = false;
        let mut record_min_ttl_seconds = clamp_min_ttl_seconds;

        let mut process_record = |record: Record| -> Record {
            let ttl = record.ttl();

            let ttl = std::cmp::max(ttl, clamp_min_ttl_seconds);
            let ttl = std::cmp::min(ttl, clamp_max_ttl_seconds);

            if (!found_record_ttl) || (ttl < record_min_ttl_seconds) {
                record_min_ttl_seconds = ttl;
                found_record_ttl = true;
            }

            let mut record = record;
            record.set_ttl(ttl);

            record
        };

        let mut response_message = response_message;

        for record in response_message.take_answers() {
            response_message.add_answer(process_record(record));
        }
        for record in response_message.take_name_servers() {
            response_message.add_name_server(process_record(record));
        }
        for record in response_message.take_additionals() {
            response_message.add_additional(process_record(record));
        }

        (
            Duration::from_secs(record_min_ttl_seconds.into()),
            response_message,
        )
    }

    async fn clamp_ttl_and_cache_response(
        &self,
        request_key: RequestKey,
        response_message: Message,
    ) -> Message {
        if !((response_message.response_code() == trust_dns_proto::op::ResponseCode::NoError)
            || (response_message.response_code() == trust_dns_proto::op::ResponseCode::NXDomain))
        {
            return response_message;
        }

        let (min_ttl_duration, response_message) =
            self.clamp_and_get_min_ttl_duration(response_message);

        if min_ttl_duration.as_secs() == 0 {
            return response_message;
        }

        let now = Instant::now();

        let mut response_message_clone = response_message.clone();
        response_message_clone.set_id(0);
        self.cache
            .put(
                request_key,
                CacheObject::new(response_message_clone, now, min_ttl_duration),
            )
            .await;

        response_message
    }

    fn get_message_for_local_domain(
        &self,
        request_key: &RequestKey,
        request_id: u16,
    ) -> Option<Message> {
        let mut response_message = match self.local_domain_cache.get_response_message(&request_key)
        {
            None => return None,
            Some(message) => message,
        };

        response_message.set_id(request_id);

        Some(response_message)
    }

    async fn get_message_for_cache_hit(
        &self,
        request_key: &RequestKey,
        request_id: u16,
    ) -> Option<Message> {
        let cache_object = match self.cache.get(&request_key).await {
            None => return None,
            Some(cache_object) => cache_object,
        };

        let now = Instant::now();

        if cache_object.expired(now) {
            return None;
        }

        let seconds_to_subtract_from_ttl = cache_object.duration_in_cache(now).as_secs();

        let adjust_record_ttl = |record: Record| -> Option<Record> {
            let original_ttl = u64::from(record.ttl());

            if seconds_to_subtract_from_ttl > original_ttl {
                None
            } else {
                let new_ttl = original_ttl - seconds_to_subtract_from_ttl;
                let new_ttl = match u32::try_from(new_ttl) {
                    Ok(new_ttl) => new_ttl,
                    Err(e) => {
                        warn!(
                            "get_message_for_cache_hit new_ttl overflow {} {}",
                            new_ttl, e
                        );
                        return None;
                    }
                };
                let mut record = record;
                record.set_ttl(new_ttl);
                Some(record)
            }
        };

        let mut cache_object = cache_object;
        let response_message = cache_object.message_mut();

        for record in response_message.take_answers() {
            response_message.add_answer(match adjust_record_ttl(record) {
                None => return None,
                Some(record) => record,
            });
        }
        for record in response_message.take_name_servers() {
            response_message.add_name_server(match adjust_record_ttl(record) {
                None => return None,
                Some(record) => record,
            });
        }
        for record in response_message.take_additionals() {
            response_message.add_additional(match adjust_record_ttl(record) {
                None => return None,
                Some(record) => record,
            });
        }

        response_message.set_id(request_id);

        Some(cache_object.message())
    }

    async fn process_request_message(&self, request_message: &Message) -> Message {
        debug!(
            "process_request_message request_message {:#?}",
            request_message
        );

        let request_key = match RequestKey::try_from(request_message) {
            Ok(request_key) => request_key,
            Err(e) => {
                warn!("request_key try_from error: {}", e);
                return self.build_failure_response_message(request_message);
            }
        };

        debug!("request_key = {:#?}", request_key);

        if let Some(response_message) =
        self.get_message_for_local_domain(&request_key, request_message.header().id())
        {
            debug!("local domain request");
            self.metrics.local_requests().increment_value();
            return response_message;
        }

        if let Some(response_message) = self
            .get_message_for_cache_hit(&request_key, request_message.header().id())
            .await
        {
            debug!("cache hit");
            self.metrics.cache_hits().increment_value();
            return response_message;
        }

        debug!("cache miss");
        self.metrics.cache_misses().increment_value();

        let response_message = match self.make_doh_request(request_message).await {
            None => return self.build_failure_response_message(request_message),
            Some(response_message) => response_message,
        };

        let mut response_message = self
            .clamp_ttl_and_cache_response(request_key, response_message)
            .await;
        response_message.set_id(request_message.header().id());

        response_message
    }

    pub(in crate::doh) async fn process_request_packet_buffer(
        &self,
        request_buffer: Vec<u8>,
    ) -> Option<Vec<u8>> {
        debug!(
            "process_request_packet_buffer received {}",
            request_buffer.len()
        );

        let request_message = match utils::decode_dns_message(request_buffer) {
            Err(e) => {
                warn!("decode_dns_message request error {}", e);
                return None;
            }
            Ok(message) => message,
        };

        let response_message = self.process_request_message(&request_message).await;

        match utils::encode_dns_message(response_message) {
            Err(e) => {
                warn!("encode_dns_message response error {}", e);
                self.build_failure_response_buffer(&request_message)
            }
            Ok(buffer) => Some(buffer),
        }
    }

    async fn run_periodic_timer(self: Arc<Self>) {
        info!("begin run_periodic_timer");

        let timer_duration = Duration::from_secs(self.configuration.timer_interval_seconds());

        loop {
            tokio::time::delay_for(timer_duration).await;

            let (cache_len, cache_items_purged) = self.cache.periodic_purge().await;

            info!("{} cache_len={} cache_items_purged={}",
                  self.metrics.all_metrics_string(), cache_len, cache_items_purged,
            );
        }
    }

    pub async fn run(self: Arc<Self>) -> Result<(), Box<dyn Error>> {
        info!("begin run");

        tokio::spawn(Arc::clone(&self).run_periodic_timer());

        let tcp_server = crate::doh::tcpserver::TCPServer::new(
            self.configuration.server_configuration().clone(),
            Arc::clone(&self.metrics),
            Arc::clone(&self),
        );
        tokio::spawn(async move {
            if let Err(e) = tcp_server.run().await {
                panic!("run_tcp_server returned error {}", e);
            }
        });

        let udp_server = crate::doh::udpserver::UDPServer::new(
            self.configuration.server_configuration().clone(),
            Arc::clone(&self.metrics),
            Arc::clone(&self),
        );
        udp_server.run().await
    }
}
