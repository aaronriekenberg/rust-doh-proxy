mod doh;

use log::info;

use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::builder().format_timestamp(None).init();

    info!(
        "Build SEMVER: {} Timestamp: {} SHA: {}",
        env!("VERGEN_SEMVER_LIGHTWEIGHT"),
        env!("VERGEN_BUILD_TIMESTAMP"),
        env!("VERGEN_SHA")
    );

    let config_file = std::env::args()
        .nth(1)
        .ok_or("config file required as command line argument")?;

    let configuration = doh::config::read_configuration(config_file).await?;

    let doh_proxy = doh::proxy::DOHProxy::new(configuration)?;

    doh_proxy.run().await
}
