use bytes::Buf;

use crate::doh::config::ClientConfiguration;

use log::debug;

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
    client_configuration: ClientConfiguration,
    hyper_client: HyperClient,
}

impl DOHClient {
    pub fn new(client_configuration: ClientConfiguration) -> Self {
        let https = hyper_tls::HttpsConnector::new();
        DOHClient {
            client_configuration,
            hyper_client: hyper::Client::builder().build::<_, hyper::Body>(https),
        }
    }

    pub async fn make_doh_request(
        &self,
        request_buffer: Vec<u8>,
    ) -> Result<DOHResponse, Box<dyn Error>> {
        let request = hyper::Request::builder()
            .method("POST")
            .uri(self.client_configuration.remote_url())
            .header("Content-Type", "application/dns-message")
            .header("Accept", "application/dns-message")
            .body(hyper::Body::from(request_buffer))?;

        let response = self.hyper_client.request(request).await?;

        debug!("after hyper post response status = {}", response.status());

        if response.status() != hyper::StatusCode::OK {
            return Ok(DOHResponse::HTTPRequestError);
        }

        let body = hyper::body::aggregate(response).await?;
        let body_vec = body.bytes().to_vec();
        Ok(DOHResponse::HTTPRequestSuccess(body_vec))
    }
}
