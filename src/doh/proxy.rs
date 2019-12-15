use crate::doh::cache::{get_cache_key, Cache, CacheObject};
use crate::doh::client::DOHClient;

use log::{debug, info, warn};

use std::error::Error;
use std::sync::Arc;

use trust_dns_proto::error::ProtoResult;
use trust_dns_proto::op::Message;
use trust_dns_proto::serialize::binary::{BinDecodable, BinDecoder, BinEncodable, BinEncoder};

pub struct DOHProxy {
    cache: Cache,
    doh_client: DOHClient,
}

impl DOHProxy {
    pub fn new() -> Arc<Self> {
        Arc::new(DOHProxy {
            cache: Cache::new(),
            doh_client: DOHClient::new(),
        })
    }

    fn encode_dns_message(&self, message: &Message) -> ProtoResult<Vec<u8>> {
        let mut request_buffer = Vec::new();

        let mut encoder = BinEncoder::new(&mut request_buffer);
        match message.emit(&mut encoder) {
            Ok(()) => {
                info!(
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

        info!("got response_buffer length = {}", response_buffer.len());

        let response_message = match self.decode_dns_message_vec(response_buffer) {
            Err(e) => {
                warn!("decode_dns_message error {}", e);
                return None;
            }
            Ok(message) => message,
        };

        Some(response_message)
    }

    async fn process_request_message(&self, request_message: &Message) -> Message {
        if request_message.queries().is_empty() {
            info!("request_message.queries is empty");
            return self.build_failure_response_message(&request_message);
        }

        let cache_key = get_cache_key(&request_message);
        info!("cache_key = '{}'", cache_key);

        if let Some(mut cache_object) = self.cache.get(&cache_key).await {
            info!("cache hit");

            cache_object.message.set_id(request_message.header().id());

            return cache_object.message;
        }

        let mut doh_request_message = request_message.clone();
        doh_request_message.set_id(0);

        let response_message = match self.make_doh_request(doh_request_message).await {
            None => return self.build_failure_response_message(&request_message),
            Some(response_message) => response_message,
        };

        if (cache_key.len() > 0)
            && ((response_message.response_code() == trust_dns_proto::op::ResponseCode::NoError)
                || (response_message.response_code()
                    == trust_dns_proto::op::ResponseCode::NXDomain))
        {
            info!("caching response");
            let new_cache_size = self
                .cache
                .put(cache_key, CacheObject::new(response_message.clone()))
                .await;
            info!("new_cache_size = {}", new_cache_size);
        }

        // info!("response_message = {:#?}", response_message);

        let mut response_message = response_message;
        response_message.set_id(request_message.header().id());

        response_message
    }

    pub(in crate::doh) async fn process_request_packet_buffer(
        &self,
        request_buffer: &[u8],
    ) -> Option<Vec<u8>> {
        info!(
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

        debug!(
            "process_request_packet_buffer request_message {:#?}",
            request_message
        );

        let response_message = self.process_request_message(&request_message).await;

        match self.encode_dns_message(&response_message) {
            Err(e) => {
                warn!("encode_dns_message response error {}", e);
                self.build_failure_response_buffer(&request_message)
            }
            Ok(buffer) => Some(buffer),
        }
    }

    pub async fn run(self: Arc<Self>) -> Result<(), Box<dyn Error>> {
        info!("begin DOHProxy.run");

        let tcp_server = crate::doh::tcpserver::TCPServer::new(Arc::clone(&self));
        tokio::spawn(async move {
            if let Err(e) = tcp_server.run().await {
                warn!("run_tcp_server returned error {}", e);
            }
        });

        let udp_server = crate::doh::udpserver::UDPServer::new(Arc::clone(&self));
        udp_server.run().await
    }
}
