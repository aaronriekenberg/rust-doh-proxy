use log::info;

use serde_derive::Deserialize;

use std::error::Error;

use tokio::fs::File;
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfiguration {
    listen_address: String,
    udp_response_channel_capacity: usize,
}

impl ServerConfiguration {
    pub fn listen_address(&self) -> &String {
        &self.listen_address
    }

    pub fn udp_response_channel_capacity(&self) -> usize {
        self.udp_response_channel_capacity
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ForwardDomainConfiguration {
    name: String,
    ip_address: String,
    ttl_seconds: u32,
}

impl ForwardDomainConfiguration {
    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn ip_address(&self) -> &String {
        &self.ip_address
    }

    pub fn ttl_seconds(&self) -> u32 {
        self.ttl_seconds
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ReverseDomainConfiguration {
    reverse_address: String,
    name: String,
    ttl_seconds: u32,
}

impl ReverseDomainConfiguration {
    pub fn reverse_address(&self) -> &String {
        &self.reverse_address
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn ttl_seconds(&self) -> u32 {
        self.ttl_seconds
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CacheConfiguration {
    max_size: usize,
    max_purges_per_timer_pop: usize,
}

impl CacheConfiguration {
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    pub fn max_purges_per_timer_pop(&self) -> usize {
        self.max_purges_per_timer_pop
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ClientConfiguration {
    remote_url: String,
    request_timeout_seconds: u64,
    max_outstanding_requests: usize,
}

impl ClientConfiguration {
    pub fn remote_url(&self) -> &String {
        &self.remote_url
    }

    pub fn request_timeout_seconds(&self) -> u64 {
        self.request_timeout_seconds
    }

    pub fn max_outstanding_requests(&self) -> usize {
        self.max_outstanding_requests
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfiguration {
    clamp_min_ttl_seconds: u32,
    clamp_max_ttl_seconds: u32,
}

impl ProxyConfiguration {
    pub fn clamp_min_ttl_seconds(&self) -> u32 {
        self.clamp_min_ttl_seconds
    }

    pub fn clamp_max_ttl_seconds(&self) -> u32 {
        self.clamp_max_ttl_seconds
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Configuration {
    server_configuration: ServerConfiguration,
    forward_domain_configurations: Vec<ForwardDomainConfiguration>,
    reverse_domain_configurations: Vec<ReverseDomainConfiguration>,
    cache_configuration: CacheConfiguration,
    client_configuration: ClientConfiguration,
    proxy_configuration: ProxyConfiguration,
    timer_interval_seconds: u64,
}

impl Configuration {
    pub fn server_configuration(&self) -> &ServerConfiguration {
        &self.server_configuration
    }

    pub fn forward_domain_configurations(&self) -> &Vec<ForwardDomainConfiguration> {
        &self.forward_domain_configurations
    }

    pub fn reverse_domain_configurations(&self) -> &Vec<ReverseDomainConfiguration> {
        &self.reverse_domain_configurations
    }

    pub fn cache_configuration(&self) -> &CacheConfiguration {
        &self.cache_configuration
    }

    pub fn client_configuration(&self) -> &ClientConfiguration {
        &self.client_configuration
    }

    pub fn proxy_configuration(&self) -> &ProxyConfiguration {
        &self.proxy_configuration
    }

    pub fn timer_interval_seconds(&self) -> u64 {
        self.timer_interval_seconds
    }
}

pub async fn read_configuration(config_file: String) -> Result<Configuration, Box<dyn Error>> {
    info!("reading {}", config_file);

    let mut file = File::open(config_file).await?;

    let mut file_contents = Vec::new();

    file.read_to_end(&mut file_contents).await?;

    let configuration: Configuration = ::serde_json::from_slice(&file_contents)?;

    info!("read_configuration configuration\n{:#?}", configuration);

    Ok(configuration)
}
