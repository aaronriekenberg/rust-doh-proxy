use std::error::Error;
use std::fmt;
use std::time::Duration;

use bytes::Buf;
use log::{debug, warn};
use tokio::sync::{Semaphore, SemaphorePermit};

use crate::doh::config::ClientConfiguration;

#[derive(Debug)]
enum DOHRequestErrorType {
    TooManyOutstandingRequests,
    HTTPRequestError,
    ContentLengthTooLong,
    ContentLengthMissing,
}

#[derive(Debug)]
struct DOHRequestError {
    error_type: DOHRequestErrorType,
}

impl DOHRequestError {
    fn new(error_type: DOHRequestErrorType) -> Box<Self> {
        Box::new(DOHRequestError { error_type })
    }
}

impl fmt::Display for DOHRequestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self.error_type {
                DOHRequestErrorType::TooManyOutstandingRequests => "too many outstanding requests",
                DOHRequestErrorType::HTTPRequestError => "http request error",
                DOHRequestErrorType::ContentLengthTooLong => "content length too long",
                DOHRequestErrorType::ContentLengthMissing => "content length missing"
            }
        )
    }
}

impl Error for DOHRequestError {}

const MAX_CONTENT_LENGTH: u64 = 65_535; // RFC 8484 section 6

fn validate_response_content_length(
    content_length_option: Option<u64>,
) -> Result<(), Box<dyn Error>> {
    match content_length_option {
        Some(content_length) => {
            debug!("content_length = {}", content_length);
            if content_length > MAX_CONTENT_LENGTH {
                warn!("got too long response content_length = {}", content_length);
                Err(DOHRequestError::new(
                    DOHRequestErrorType::ContentLengthTooLong,
                ))
            } else {
                Ok(())
            }
        }
        None => {
            warn!("content_length = None");
            Err(DOHRequestError::new(
                DOHRequestErrorType::ContentLengthMissing,
            ))
        }
    }
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

    fn acquire_semaphore(&self) -> Result<SemaphorePermit<'_>, Box<DOHRequestError>> {
        self.request_semaphore.try_acquire().map_err(|_| DOHRequestError::new(
            DOHRequestErrorType::TooManyOutstandingRequests,
        ))
    }

    pub async fn make_doh_request(
        &self,
        request_buffer: Vec<u8>,
    ) -> Result<Vec<u8>, Box<dyn Error>> {
        let _permit = self.acquire_semaphore()?;

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
            warn!("got http error response status {}", response.status().as_u16());
            return Err(DOHRequestError::new(DOHRequestErrorType::HTTPRequestError));
        }

        validate_response_content_length(response.content_length())?;

        let body = response.bytes().await?;

        let body_vec = body.bytes().to_vec();
        Ok(body_vec)
    }
}
