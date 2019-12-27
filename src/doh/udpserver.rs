use crate::doh::config::ServerConfiguration;
use crate::doh::proxy::DOHProxy;

use log::{debug, info, warn};

use std::error::Error;
use std::sync::Arc;

use tokio::net::{udp::SendHalf, UdpSocket};
use tokio::sync::mpsc;

struct UDPResponseMessage(Vec<u8>, std::net::SocketAddr);

pub struct UDPServer {
    server_configuration: ServerConfiguration,
    doh_proxy: Arc<DOHProxy>,
}

impl UDPServer {
    pub fn new(server_configuration: ServerConfiguration, doh_proxy: Arc<DOHProxy>) -> Arc<Self> {
        Arc::new(UDPServer {
            server_configuration,
            doh_proxy,
        })
    }

    async fn process_udp_packet(
        self: Arc<Self>,
        response_sender: mpsc::UnboundedSender<UDPResponseMessage>,
        request_buffer: Vec<u8>,
        request_bytes_received: usize,
        peer: std::net::SocketAddr,
    ) {
        match self
            .doh_proxy
            .process_request_packet_buffer(&request_buffer[..request_bytes_received])
            .await
        {
            Some(response_buffer) => {
                let response_message = UDPResponseMessage(response_buffer, peer);
                match response_sender.send(response_message) {
                    Err(e) => warn!("response_sender.send error {}", e),
                    Ok(_) => debug!("response_sender.send success"),
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
                    warn!("run_udp_response_sender received none");
                    break;
                }
                Some(msg) => match socket_send_half.send_to(&msg.0, &msg.1).await {
                    Ok(bytes_sent) => debug!("send_to success bytes_sent {}", bytes_sent),
                    Err(e) => warn!("send_to error {}", e),
                },
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
            let (bytes_received, peer) = socket_recv_half.recv_from(&mut buf).await?;

            tokio::spawn(Arc::clone(&self).process_udp_packet(
                response_sender.clone(),
                buf,
                bytes_received,
                peer,
            ));
        }
    }
}
