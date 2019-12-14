use bytes::Buf;

use log::info;

use std::error::Error;

pub enum DOHResponse {
    HTTPRequestError,
    HTTPRequestSuccess(Vec<u8>),
}

type HyperClient = hyper::client::Client<
    hyper_tls::HttpsConnector<hyper::client::connect::HttpConnector>,
    hyper::Body,
>;

pub struct DOHClient {
    hyper_client: HyperClient,
}

impl DOHClient {
    pub fn new() -> Self {
        let https = hyper_tls::HttpsConnector::new();
        DOHClient {
            hyper_client: hyper::Client::builder().build::<_, hyper::Body>(https),
        }
    }

    pub async fn make_doh_request(
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
}
