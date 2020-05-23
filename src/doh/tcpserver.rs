use std::convert::TryFrom;
use std::error::Error;
use std::sync::Arc;

use log::{debug, info, warn};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::doh::config::ServerConfiguration;
use crate::doh::metrics::Metrics;
use crate::doh::proxy::DOHProxy;

pub struct TCPServer {
    server_configuration: ServerConfiguration,
    metrics: Arc<Metrics>,
    doh_proxy: Arc<DOHProxy>,
}

impl TCPServer {
    pub fn new(
        server_configuration: ServerConfiguration,
        metrics: Arc<Metrics>,
        doh_proxy: Arc<DOHProxy>,
    ) -> Arc<Self> {
        Arc::new(TCPServer {
            server_configuration,
            metrics,
            doh_proxy,
        })
    }

    async fn process_tcp_stream(
        self: Arc<Self>,
        mut stream: TcpStream,
    ) -> Result<(), Box<dyn Error>> {
        loop {
            let mut buffer = [0u8; 2];
            stream.read_exact(&mut buffer).await?;
            let length = u16::from_be_bytes(buffer);

            if length == 0 {
                warn!("read 0 length tcp header");
                break;
            }

            let mut buffer = vec![0u8; usize::from(length)];
            stream.read_exact(&mut buffer).await?;

            self.metrics.tcp_requests().increment_value();

            let buffer = match self.doh_proxy.process_request_packet_buffer(buffer).await {
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

    pub async fn run(self: Arc<Self>) -> Result<(), Box<dyn Error>> {
        info!("begin run");

        let mut listener = TcpListener::bind(self.server_configuration.listen_address()).await?;
        info!("listening on tcp {}", listener.local_addr()?);

        loop {
            let (stream, peer_addr) = match listener.accept().await {
                Err(e) => {
                    warn!("tcp accept error {}", e);
                    continue;
                }
                Ok(result) => result,
            };
            debug!("accepted tcp connection from {}", peer_addr);

            let self_clone = Arc::clone(&self);
            tokio::spawn(async move {
                if let Err(e) = self_clone.process_tcp_stream(stream).await {
                    debug!("process_tcp_stream returnd error {}", e);
                }
            });
        }
    }
}
