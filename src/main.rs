mod doh;

use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::init();

    let doh_proxy = doh::proxy::DOHProxy::new();

    doh_proxy.run().await
}
