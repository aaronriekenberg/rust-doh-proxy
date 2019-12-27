use log::info;

use serde_derive::Deserialize;

use std::error::Error;
use std::io::Read;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfiguration {
    listen_address: String,
}

impl ServerConfiguration {
    pub fn listen_address(&self) -> &String {
        &self.listen_address
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Configuration {
    server_configuration: ServerConfiguration,
    timer_interval_seconds: u64,
}

impl Configuration {
    pub fn server_configuration(&self) -> &ServerConfiguration {
        &self.server_configuration
    }

    pub fn timer_interval_seconds(&self) -> u64 {
        self.timer_interval_seconds
    }
}

pub fn read_configuration(config_file: String) -> Result<Configuration, Box<dyn Error>> {
    info!("reading {}", config_file);

    let mut file = ::std::fs::File::open(config_file)?;

    let mut file_contents = String::new();

    file.read_to_string(&mut file_contents)?;

    let configuration: Configuration = ::serde_json::from_str(&file_contents)?;

    info!("read_configuration configuration\n{:#?}", configuration);

    Ok(configuration)
}
