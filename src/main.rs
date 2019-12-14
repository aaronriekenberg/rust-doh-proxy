use bytes::Buf;

use log::{debug, info, warn};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{udp::SendHalf, TcpListener, TcpStream, UdpSocket};
use tokio::sync::mpsc;

use std::convert::TryFrom;
use std::error::Error;
use std::sync::Arc;

use trust_dns_proto::error::ProtoResult;
use trust_dns_proto::op::Message;
use trust_dns_proto::serialize::binary::{BinDecodable, BinDecoder, BinEncodable, BinEncoder};

enum DOHResponse {
    HTTPRequestError,
    HTTPRequestSuccess(Vec<u8>),
}

type HyperClient = hyper::client::Client<
    hyper_tls::HttpsConnector<hyper::client::connect::HttpConnector>,
    hyper::Body,
>;

pub struct DOHProxy {
    hyper_client: HyperClient,
}

struct UDPResponseMessage(Vec<u8>, std::net::SocketAddr);

impl DOHProxy {
    pub fn new() -> Arc<Self> {
        let https = hyper_tls::HttpsConnector::new();

        Arc::new(DOHProxy {
            hyper_client: hyper::Client::builder().build::<_, hyper::Body>(https),
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

    async fn make_doh_request(
        &self,
        request_buffer: Vec<u8>,
    ) -> Result<DOHResponse, Box<dyn Error>> {
        info!("make_doh_request");

        let request = hyper::Request::builder()
            .method("POST")
            .uri("https://cloudflare-dns.com/dns-query")
            .header("Content-Type", "application/dns-message")
            .header("Accept", "application/dns-message")
            .body(hyper::Body::from(request_buffer))?;

        let response = self.hyper_client.request(request).await?;

        info!("after hyper post response status = {}", response.status());

        if response.status() != hyper::StatusCode::OK {
            return Ok(DOHResponse::HTTPRequestError);
        }

        let body = hyper::body::aggregate(response).await?;
        let body_vec = body.bytes().to_vec();
        Ok(DOHResponse::HTTPRequestSuccess(body_vec))
    }

    async fn process_request_packet_buffer(&self, request_buffer: &[u8]) -> Option<Vec<u8>> {
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

        let doh_response = match self.make_doh_request(request_buffer).await {
            Err(e) => {
                warn!("make_doh_request error {}", e);
                return self.build_failure_response(&request_message);
            }
            Ok(doh_response) => doh_response,
        };

        let response_buffer = match doh_response {
            DOHResponse::HTTPRequestError => {
                warn!("got http request error");
                return self.build_failure_response(&request_message);
            }
            DOHResponse::HTTPRequestSuccess(response_buffer) => response_buffer,
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

    async fn process_tcp_stream(
        self: Arc<Self>,
        mut stream: TcpStream,
    ) -> Result<(), Box<dyn Error>> {
        info!("process_tcp_stream peer_addr = {}", stream.peer_addr()?);

        loop {
            let mut buffer = [0u8; 2];
            stream.read_exact(&mut buffer).await?;
            let length = u16::from_be_bytes(buffer);
            info!("request length = {}", length);

            let mut buffer = vec![0u8; usize::from(length)];
            stream.read_exact(&mut buffer).await?;
            info!("read request buffer len = {}", buffer.len());

            let buffer = match self.process_request_packet_buffer(&buffer).await {
                Some(buffer) => buffer,
                None => {
                    warn!("got None response from process_request_packet_buffer");
                    continue;
                }
            };

            let length = match u16::try_from(buffer.len()) {
                Ok(len) => len,
                Err(e) => {
                    warn!("response buffer.len overflow {}: {}", buffer.len(), e);
                    break;
                }
            };

            stream.write_all(&length.to_be_bytes()).await?;
            stream.write_all(&buffer).await?;
        }

        Ok(())
    }

    async fn run_tcp_server(self: Arc<Self>) -> Result<(), Box<dyn Error>> {
        info!("begin run_tcp_server");

        let mut listener = TcpListener::bind("127.0.0.1:10053").await?;
        info!("Listening on tcp {}", listener.local_addr()?);

        loop {
            let (stream, _) = listener.accept().await?;
            let self_clone = Arc::clone(&self);
            tokio::spawn(async move {
                if let Err(e) = self_clone.process_tcp_stream(stream).await {
                    debug!("process_tcp_stream returnd error {}", e);
                }
            });
        }
    }

    async fn process_udp_packet(
        self: Arc<Self>,
        response_sender: mpsc::UnboundedSender<UDPResponseMessage>,
        request_buffer: Vec<u8>,
        request_bytes_received: usize,
        peer: std::net::SocketAddr,
    ) {
        match self
            .process_request_packet_buffer(&request_buffer[..request_bytes_received])
            .await
        {
            Some(response_buffer) => {
                let response_message = UDPResponseMessage(response_buffer, peer);
                match response_sender.send(response_message) {
                    Err(e) => warn!("response_sender.send error {}", e),
                    Ok(_) => info!("response_sender.send success"),
                }
            }
            None => warn!("got None response from process_request_packet_buffer"),
        }
    }

    async fn run_udp_response_sender(
        self: Arc<Self>,
        mut response_receiver: mpsc::UnboundedReceiver<UDPResponseMessage>,
        mut socket_send_half: SendHalf,
    ) {
        info!("begin run_udp_response_sender");
        loop {
            match response_receiver.recv().await {
                None => {
                    info!("received none");
                    break;
                }
                Some(msg) => {
                    info!("received msg");
                    match socket_send_half.send_to(&msg.0, &msg.1).await {
                        Ok(bytes_sent) => info!("send_to success bytes_sent {}", bytes_sent),
                        Err(e) => warn!("send_to error {}", e),
                    }
                }
            }
        }
    }

    async fn run_udp_server(self: Arc<Self>) -> Result<(), Box<dyn Error>> {
        info!("begin run_udp_server");

        let (response_sender, response_receiver) = mpsc::unbounded_channel::<UDPResponseMessage>();

        let socket = UdpSocket::bind("127.0.0.1:10053").await?;

        info!("Listening on udp {}", socket.local_addr()?);

        let (mut socket_recv_half, socket_send_half) = socket.split();

        tokio::spawn(
            Arc::clone(&self).run_udp_response_sender(response_receiver, socket_send_half),
        );

        loop {
            let mut buf = vec![0u8; 2048];
            let (bytes_received, peer) = socket_recv_half.recv_from(&mut buf).await?;

            tokio::spawn(Arc::clone(&self).process_udp_packet(
                response_sender.clone(),
                buf,
                bytes_received,
                peer,
            ));
        }
    }

    pub async fn run_server(self: Arc<Self>) -> Result<(), Box<dyn Error>> {
        info!("begin run_server");

        let self_clone = Arc::clone(&self);
        tokio::spawn(async move {
            if let Err(e) = self_clone.run_tcp_server().await {
                warn!("run_tcp_server returned error {}", e);
            }
        });

        self.run_udp_server().await
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let doh_proxy = DOHProxy::new();

    doh_proxy.run_server().await
}
