use async_std::io;
use async_std::net::{TcpListener, TcpStream, UdpSocket};
use async_std::prelude::*;
use async_std::task;

use log::{info, warn};

use std::convert::TryFrom;
use std::sync::Arc;

use trust_dns_proto::error::ProtoResult;
use trust_dns_proto::op::Message;
use trust_dns_proto::serialize::binary::{BinDecodable, BinDecoder, BinEncodable, BinEncoder};

fn encode_dns_message(message: &Message) -> ProtoResult<Vec<u8>> {
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

fn decode_dns_message(buffer: Vec<u8>) -> ProtoResult<Message> {
    let mut decoder = BinDecoder::new(&buffer);
    match Message::read(&mut decoder) {
        Ok(message) => Ok(message),
        Err(e) => {
            warn!("error decoding dns message {}", e);
            Err(e)
        }
    }
}

fn build_failure_response(request: &Message) -> Option<Vec<u8>> {
    let mut response_message = request.clone();
    response_message.set_message_type(trust_dns_proto::op::MessageType::Response);
    response_message.set_response_code(trust_dns_proto::op::ResponseCode::ServFail);

    match encode_dns_message(&response_message) {
        Err(e) => {
            warn!("encode_dns_message error {}", e);
            None
        }
        Ok(buffer) => Some(buffer),
    }
}

enum DOHResponse {
    HTTPRequestError,
    HTTPRequestSuccess(Vec<u8>),
}

async fn make_doh_request(request_buffer: Vec<u8>) -> Result<DOHResponse, surf::Exception> {
    info!("make_doh_request");

    info!("before surf post");

    let mut response = surf::post("https://dns.google/dns-query")
        .body_bytes(request_buffer)
        .set_header("content-type", "application/dns-message")
        .set_header("accept", "application/dns-message")
        .await?;

    info!("after surf post response status = {}", response.status());

    if response.status() != 200 {
        return Ok(DOHResponse::HTTPRequestError);
    }

    let response_buffer = response.body_bytes().await?;
    Ok(DOHResponse::HTTPRequestSuccess(response_buffer))
}

async fn process_request_packet_buffer(request_buffer: &[u8]) -> Option<Vec<u8>> {
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

    // info!("parsed udp dns packet {:#?}", request_message);

    if request_message.queries().len() < 1 {
        info!("request_message.queries is empty");
        return build_failure_response(&request_message);
    }

    let mut doh_request_message = request_message.clone();
    doh_request_message.set_id(0);
    let doh_request_message = doh_request_message;
    let request_buffer = match encode_dns_message(&doh_request_message) {
        Err(e) => {
            warn!("encode_dns_message error {}", e);
            return build_failure_response(&request_message);
        }
        Ok(buffer) => buffer,
    };

    let doh_response = match make_doh_request(request_buffer).await {
        Err(e) => {
            warn!("make_doh_request error {}", e);
            return build_failure_response(&request_message);
        }
        Ok(doh_response) => doh_response,
    };

    let response_buffer = match doh_response {
        DOHResponse::HTTPRequestError => {
            warn!("got http request error");
            return build_failure_response(&request_message);
        }
        DOHResponse::HTTPRequestSuccess(response_buffer) => response_buffer,
    };

    info!("got response_buffer length = {}", response_buffer.len());

    let mut response_message = match decode_dns_message(response_buffer) {
        Err(e) => {
            warn!("decode_dns_message error {}", e);
            return build_failure_response(&request_message);
        }
        Ok(message) => message,
    };

    // info!("response_message = {:#?}", response_message);

    response_message.set_id(request_message.header().id());

    let response_buffer = match encode_dns_message(&response_message) {
        Err(e) => {
            warn!("encode_dns_message error {}", e);
            return build_failure_response(&request_message);
        }
        Ok(buffer) => buffer,
    };

    Some(response_buffer)
}

async fn process_udp_packet(
    socket: Arc<UdpSocket>,
    request_buffer: Vec<u8>,
    request_bytes_received: usize,
    peer: std::net::SocketAddr,
) {
    match process_request_packet_buffer(&request_buffer[..request_bytes_received]).await {
        Some(response_buffer) => match socket.send_to(&response_buffer, peer).await {
            Err(e) => warn!("send_to error {}", e),
            Ok(bytes_written) => info!("send_to success bytes_written = {}", bytes_written),
        },
        None => warn!("got None response from process_request_packet_buffer"),
    }
}

async fn process_tcp_stream(stream: TcpStream) -> io::Result<()> {
    info!("process_tcp_stream peer_addr = {}", stream.peer_addr()?);

    let (reader, writer) = &mut (&stream, &stream);

    loop {
        let mut buffer = [0u8; 2];
        reader.read_exact(&mut buffer).await?;

        let length = u16::from_be_bytes(buffer);
        info!("request length = {}", length);

        let mut buffer = vec![0u8; usize::from(length)];
        reader.read_exact(&mut buffer).await?;

        info!("read request buffer len = {}", buffer.len());

        let buffer = match process_request_packet_buffer(&buffer).await {
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

        writer.write_all(&length.to_be_bytes()).await?;

        writer.write_all(&buffer).await?;
    }

    Ok(())
}

async fn run_tcp_server() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:10053").await?;

    info!("Listening on tcp {}", listener.local_addr()?);

    let mut incoming = listener.incoming();

    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        task::spawn(process_tcp_stream(stream));
    }
    Ok(())
}

async fn run_server() -> io::Result<()> {
    task::spawn(run_tcp_server());

    let socket = Arc::new(UdpSocket::bind("127.0.0.1:10053").await?);

    info!("Listening on udp {}", socket.local_addr()?);

    loop {
        let mut buf = vec![0u8; 2048];
        let (bytes_received, peer) = socket.recv_from(&mut buf).await?;

        task::spawn(process_udp_packet(
            Arc::clone(&socket),
            buf,
            bytes_received,
            peer,
        ));
    }
}

fn main() -> io::Result<()> {
    env_logger::init();

    let server_future = run_server();

    task::block_on(server_future)
}
