use bytes::Buf;

use crate::doh::config::ClientConfiguration;

use log::debug;

use std::error::Error;
use std::time::Duration;

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
        let timeout_duration = Duration::from_secs(client_configuration.request_timeout_seconds());
        DOHClient {
            client_configuration,
            client: reqwest::Client::builder()
                .use_rustls_tls()
                .timeout(timeout_duration)
                .build()
                .expect("error creating reqwest client"),
        }
    }

    pub async fn make_doh_request(
        &self,
        request_buffer: Vec<u8>,
    ) -> Result<DOHResponse, Box<dyn Error>> {
        let response = self
            .client
            .post(self.client_configuration.remote_url())
            .header("Content-Type", "application/dns-message")
            .header("Accept", "application/dns-message")
            .body(request_buffer)
            .send()
            .await?;

        debug!("after reqwest post response status = {}", response.status());

        if response.status() != reqwest::StatusCode::OK {
            return Ok(DOHResponse::HTTPRequestError);
        }

        let body = response.bytes().await?;

        let body_vec = body.bytes().to_vec();
        Ok(DOHResponse::HTTPRequestSuccess(body_vec))
    }
}
