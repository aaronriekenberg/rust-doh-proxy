use crate::doh::proxy::DOHProxy;

use log::{debug, info, warn};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use std::convert::TryFrom;
use std::error::Error;
use std::sync::Arc;

pub struct TCPServer {
    doh_proxy: Arc<DOHProxy>,
}

impl TCPServer {
    pub fn new(doh_proxy: Arc<DOHProxy>) -> Arc<Self> {
        Arc::new(TCPServer { doh_proxy })
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

            let buffer = match self.doh_proxy.process_request_packet_buffer(&buffer).await {
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
        info!("begin TCPServer.run");

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
}
