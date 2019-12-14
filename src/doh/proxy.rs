use log::{debug, info, warn};

use std::error::Error;
use std::sync::Arc;

use trust_dns_proto::error::ProtoResult;
use trust_dns_proto::op::Message;
use trust_dns_proto::serialize::binary::{BinDecodable, BinDecoder, BinEncodable, BinEncoder};

pub struct DOHProxy {
    doh_client: crate::doh::client::DOHClient,
}

impl DOHProxy {
    pub fn new() -> Arc<Self> {
        Arc::new(DOHProxy {
            doh_client: crate::doh::client::DOHClient::new(),
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

    fn decode_dns_message(&self, buffer: Vec<u8>) -> ProtoResult<Message> {
        let mut decoder = BinDecoder::new(&buffer);
        match Message::read(&mut decoder) {
            Ok(message) => Ok(message),
            Err(e) => {
                warn!("error decoding dns message {}", e);
                Err(e)
            }
        }
    }

    fn build_failure_response(&self, request: &Message) -> Option<Vec<u8>> {
        let mut response_message = request.clone();
        response_message.set_message_type(trust_dns_proto::op::MessageType::Response);
        response_message.set_response_code(trust_dns_proto::op::ResponseCode::ServFail);

        match self.encode_dns_message(&response_message) {
            Err(e) => {
                warn!("encode_dns_message error {}", e);
                None
            }
            Ok(buffer) => Some(buffer),
        }
    }

    pub(super) async fn process_request_packet_buffer(
        &self,
        request_buffer: &[u8],
    ) -> Option<Vec<u8>> {
        info!(
            "process_request_packet_buffer received {}",
            request_buffer.len()
        );
        let mut decoder = BinDecoder::new(&request_buffer);

        let request_message = match Message::read(&mut decoder) {
            Err(e) => {
                warn!("udp dns packet perse error {}", e);
                return None;
            }
            Ok(message) => message,
        };

        debug!(
            "process_request_packet_buffer request_message {:#?}",
            request_message
        );

        if request_message.queries().len() < 1 {
            info!("request_message.queries is empty");
            return self.build_failure_response(&request_message);
        }

        let mut doh_request_message = request_message.clone();
        doh_request_message.set_id(0);
        let doh_request_message = doh_request_message;

        let request_buffer = match self.encode_dns_message(&doh_request_message) {
            Err(e) => {
                warn!("encode_dns_message error {}", e);
                return self.build_failure_response(&request_message);
            }
            Ok(buffer) => buffer,
        };

        let doh_response = match self.doh_client.make_doh_request(request_buffer).await {
            Err(e) => {
                warn!("make_doh_request error {}", e);
                return self.build_failure_response(&request_message);
            }
            Ok(doh_response) => doh_response,
        };

        let response_buffer = match doh_response {
            crate::doh::client::DOHResponse::HTTPRequestError => {
                warn!("got http request error");
                return self.build_failure_response(&request_message);
            }
            crate::doh::client::DOHResponse::HTTPRequestSuccess(response_buffer) => response_buffer,
        };

        info!("got response_buffer length = {}", response_buffer.len());

        let mut response_message = match self.decode_dns_message(response_buffer) {
            Err(e) => {
                warn!("decode_dns_message error {}", e);
                return self.build_failure_response(&request_message);
            }
            Ok(message) => message,
        };

        // info!("response_message = {:#?}", response_message);

        response_message.set_id(request_message.header().id());

        let response_buffer = match self.encode_dns_message(&response_message) {
            Err(e) => {
                warn!("encode_dns_message error {}", e);
                return self.build_failure_response(&request_message);
            }
            Ok(buffer) => buffer,
        };

        Some(response_buffer)
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
