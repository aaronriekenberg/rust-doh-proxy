use bytes::Buf;

use crate::doh::config::ClientConfiguration;

use log::{debug, warn};

use std::error::Error;
use std::time::Duration;

use tokio::sync::Semaphore;

const MAX_CONTENT_LENGTH: u64 = 65_535; // RFC 8484 section 6

pub enum DOHResponse {
    TooManyOutstandingRequests(usize),
    HTTPRequestError,
    HTTPRequestSuccess(Vec<u8>),
}

pub struct DOHClient {
    client_configuration: ClientConfiguration,
    client: reqwest::Client,
    request_semaphore: Semaphore,
}

impl DOHClient {
    pub fn new(client_configuration: ClientConfiguration) -> Result<Self, Box<dyn Error>> {
        let timeout_duration = Duration::from_secs(client_configuration.request_timeout_seconds());
        let max_outstanding_requests = client_configuration.max_outstanding_requests();
        Ok(DOHClient {
            client_configuration,
            client: reqwest::Client::builder()
                .use_rustls_tls()
                .timeout(timeout_duration)
                .build()?,
            request_semaphore: Semaphore::new(max_outstanding_requests),
        })
    }

    pub async fn make_doh_request(
        &self,
        request_buffer: Vec<u8>,
    ) -> Result<DOHResponse, Box<dyn Error>> {
        let _permit = match self.request_semaphore.try_acquire() {
            Ok(permit) => permit,
            Err(_) => {
                return Ok(DOHResponse::TooManyOutstandingRequests(
                    self.client_configuration.max_outstanding_requests(),
                ))
            }
        };

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
            warn!("got error response status {}", response.status().as_u16());
            return Ok(DOHResponse::HTTPRequestError);
        }

        match response.content_length() {
            Some(content_length) => {
                debug!("content_length = {}", content_length);
                if content_length > MAX_CONTENT_LENGTH {
                    warn!("got too long response content_length = {}", content_length);
                    return Ok(DOHResponse::HTTPRequestError);
                }
            }
            None => {
                warn!("content_length = None");
                return Ok(DOHResponse::HTTPRequestError);
            }
        }

        let body = response.bytes().await?;

        let body_vec = body.bytes().to_vec();
        Ok(DOHResponse::HTTPRequestSuccess(body_vec))
    }
}
