use crate::doh::config::ServerConfiguration;
use crate::doh::metrics::Metrics;
use crate::doh::proxy::DOHProxy;

use log::{debug, info, warn};

use std::error::Error;
use std::sync::Arc;

use tokio::net::{udp::SendHalf, UdpSocket};
use tokio::sync::mpsc;

struct UDPResponseMessage(Vec<u8>, std::net::SocketAddr);

pub struct UDPServer {
    server_configuration: ServerConfiguration,
    metrics: Arc<Metrics>,
    doh_proxy: Arc<DOHProxy>,
}

impl UDPServer {
    pub fn new(
        server_configuration: ServerConfiguration,
        metrics: Arc<Metrics>,
        doh_proxy: Arc<DOHProxy>,
    ) -> Arc<Self> {
        Arc::new(UDPServer {
            server_configuration,
            metrics,
            doh_proxy,
        })
    }

    async fn process_udp_packet(
        self: Arc<Self>,
        response_sender: mpsc::UnboundedSender<UDPResponseMessage>,
        request_buffer: Vec<u8>,
        peer: std::net::SocketAddr,
    ) {
        self.metrics.increment_udp_requests();

        let response_buffer = match self
            .doh_proxy
            .process_request_packet_buffer(request_buffer)
            .await
        {
            None => {
                warn!("got None response from process_request_packet_buffer");
                return;
            }
            Some(response_buffer) => response_buffer,
        };

        match response_sender.send(UDPResponseMessage(response_buffer, peer)) {
            Err(e) => warn!("response_sender.send error {}", e),
            Ok(_) => debug!("response_sender.send success"),
        }
    }

    async fn run_udp_response_sender(
        self: Arc<Self>,
        mut response_receiver: mpsc::UnboundedReceiver<UDPResponseMessage>,
        mut socket_send_half: SendHalf,
    ) {
        info!("begin run_udp_response_sender");
        loop {
            let msg = match response_receiver.recv().await {
                None => {
                    warn!("run_udp_response_sender received none");
                    break;
                }
                Some(msg) => msg,
            };

            match socket_send_half.send_to(&msg.0, &msg.1).await {
                Ok(bytes_sent) => debug!("send_to success bytes_sent {}", bytes_sent),
                Err(e) => warn!("send_to error {}", e),
            }
        }
    }

    pub async fn run(self: Arc<Self>) -> Result<(), Box<dyn Error>> {
        info!("begin run");

        let (response_sender, response_receiver) = mpsc::unbounded_channel::<UDPResponseMessage>();

        let socket = UdpSocket::bind(self.server_configuration.listen_address()).await?;

        info!("listening on udp {}", socket.local_addr()?);

        let (mut socket_recv_half, socket_send_half) = socket.split();

        tokio::spawn(
            Arc::clone(&self).run_udp_response_sender(response_receiver, socket_send_half),
        );

        loop {
            let mut buf = vec![0u8; 2048];
            let (bytes_received, peer) = match socket_recv_half.recv_from(&mut buf).await {
                Err(e) => {
                    warn!("udp recv_from error {}", e);
                    continue;
                }
                Ok(result) => result,
            };
            debug!("received {} bytes from {}", bytes_received, peer);

            if bytes_received == 0 {
                continue;
            }

            buf.truncate(bytes_received);
            buf.shrink_to_fit();

            tokio::spawn(Arc::clone(&self).process_udp_packet(response_sender.clone(), buf, peer));
        }
    }
}
