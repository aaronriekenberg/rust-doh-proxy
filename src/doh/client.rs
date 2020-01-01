use bytes::Buf;

use crate::doh::config::ClientConfiguration;

use log::debug;

use std::error::Error;
use std::time::Duration;

use tokio::time::timeout;

pub enum DOHResponse {
    HTTPRequestError,
    HTTPRequestSuccess(Vec<u8>),
}

pub struct DOHClient {
    client_configuration: ClientConfiguration,
    client: reqwest::Client,
}

impl DOHClient {
    pub fn new(client_configuration: ClientConfiguration) -> Self {
        DOHClient {
            client_configuration,
            client: reqwest::Client::builder()
                .use_rustls_tls()
                .build()
                .expect("error creating reqwest client"),
        }
    }

    pub async fn make_doh_request(
        &self,
        request_buffer: Vec<u8>,
    ) -> Result<DOHResponse, Box<dyn Error>> {
        let response = timeout(
            Duration::from_secs(self.client_configuration.request_timeout_seconds()),
            self.client
                .post(self.client_configuration.remote_url())
                .header("Content-Type", "application/dns-message")
                .header("Accept", "application/dns-message")
                .body(request_buffer)
                .send(),
        )
        .await??;

        debug!("after reqwest post response status = {}", response.status());

        if response.status() != reqwest::StatusCode::OK {
            return Ok(DOHResponse::HTTPRequestError);
        }

        let body = response.bytes().await?;

        let body_vec = body.bytes().to_vec();
        Ok(DOHResponse::HTTPRequestSuccess(body_vec))
    }
}
