use async_std::io;
use async_std::net::UdpSocket;
use async_std::task;

use dns_parser::Packet;
// use futures::try_join;

use log::{info, warn};

// async fn make_http_call(uri: &str) -> Result<String, surf::Exception> {
//     info!("make_http_call uri: {}", uri);
//
//     surf::get(uri).recv_string().await
// }
//
// async fn make_http_calls() -> Result<(), surf::Exception> {
//     let uri1 = "https://httpbin.org/get";
//     let future1 = make_http_call(&uri1);
//
//     let uri2 = "https://httpbin.org/get2";
//     let future2 = make_http_call(&uri2);
//
//     info!("before try_join");
//
//     let results = try_join!(future1, future2)?;
//
//     info!("make_http_calls got results: {:#?}", results);
//
//     Ok(())
// }

async fn make_doh_request(request_packet: &Packet<'_>) -> Result<(), surf::Exception> {
    info!("make_doh_request");

    let mut dns_query_builder = dns_parser::Builder::new_query(0, true);

    dns_query_builder.add_question(
        &request_packet.questions[0].qname.to_string(),
        false,
        request_packet.questions[0].qtype,
        request_packet.questions[0].qclass,
    );

    let query_buffer = dns_query_builder.build().unwrap_or_else(|x| x);

    info!("before surf post");

    let response = surf::post("https://dns.google/dns-query")
        .body_bytes(query_buffer)
        .set_header("content-type", "application/dns-message")
        .set_header("accept", "application/dns-message")
        .await;

    info!("got response {:#?}", response);

    Ok(())
}

async fn run_server() -> io::Result<()> {
    let socket = UdpSocket::bind("127.0.0.1:10053").await?;
    let mut buf = vec![0u8; 2048];

    info!("Listening on {}", socket.local_addr()?);

    loop {
        let (n, peer) = socket.recv_from(&mut buf).await?;
        info!("received {} from udp peer {}", n, peer);

        match Packet::parse(&buf[0..n]) {
            Err(e) => warn!("udp dns packet perse error {}", e),
            Ok(mut packet) => {
                info!("parsed udp dns packet {:#?}", packet);

                let original_id = packet.header.id;

                packet.header.id = 0;

                if packet.questions.len() == 1 {
                    make_doh_request(&packet).await;
                }
            }
        }
    }
}

fn main() -> io::Result<()> {
    env_logger::init();

    let server_future = run_server();

    task::block_on(server_future)
}
