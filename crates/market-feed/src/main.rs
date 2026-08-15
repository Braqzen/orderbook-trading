mod api;
mod worker;

use eyre::Result;
use maiya::telemetry::Telemetry;
use std::{net::SocketAddr, str::FromStr};
use worker::Worker;

pub mod proto {
    tonic::include_proto!("generatorfeed");
}

#[tokio::main]
async fn main() -> Result<()> {
    // let telemetry = Telemetry::init("market-feed")?;

    let ws = std::env::var("WS")?;
    let socket = std::env::var("SOCKET")?;

    let ws = SocketAddr::from_str(&ws)?;
    let socket = SocketAddr::from_str(&socket)?;

    let mut worker = Worker::new(socket, ws);
    let result = worker.run().await;

    // telemetry.shutdown()?;

    result
}
