use crate::{
    api::GrpcServer,
    config::Config,
    proto::PriceUpdate,
    simulation::{Feed, PriceManager},
};
use eyre::Result;
use rand::random_range;
use std::{net::SocketAddr, str::FromStr, time::Duration};
use tokio::{
    select,
    signal::unix::{SignalKind, signal},
    sync::broadcast,
    task::{JoinError, JoinSet},
    time::sleep,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

/// Time between sending a new price (milli seconds)
const PUBLISH_INTERVAL: u64 = 10;

/// Max random delay before spawning each feed task (milli seconds)
const SPAWN_JITTER_MS: u64 = 5;

pub struct Worker {
    /// Socket the generator serves price updates on. The aggregator connects as the client.
    socket: SocketAddr,
    /// Math to create a new price per config constraints
    price_managers: Vec<PriceManager>,
}

impl Worker {
    pub fn new(socket: String, config: Config) -> Result<Self> {
        let price_managers = config
            .feeds
            .iter()
            .map(|feed| {
                let instrument = feed.instrument()?;
                let price_config = feed.price_config()?;
                Ok(PriceManager::new(instrument, price_config))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            socket: SocketAddr::from_str(&socket)?,
            price_managers,
        })
    }

    pub async fn run(self) -> Result<()> {
        // Handle running locally and interrupting the process with ctrl+c.
        let mut sigint = signal(SignalKind::interrupt())?;
        // Handle running in a container and terminating the process with docker stop.
        let mut sigterm = signal(SignalKind::terminate())?;

        let token = CancellationToken::new();
        let mut tasks = JoinSet::new();

        // Each instrument feed sends prices through this channel
        // When clients connect they receive a receiver channel handle to receive these updates
        let (price_sender, _) = broadcast::channel::<PriceUpdate>(128);

        // Serve generated prices to the aggregator over gRPC
        let server_token = token.child_token();
        let server = GrpcServer::new(self.socket, price_sender.clone());
        tasks.spawn(server.run(server_token));

        // Spawn a feed per instrument with a slight delay to stagger updates
        // Real events do not all arrive at the same time each tick
        for price_manager in self.price_managers.into_iter() {
            sleep(Duration::from_millis(random_range(0..SPAWN_JITTER_MS))).await;

            let feed_token = token.child_token();
            let price_sender_channel = price_sender.clone();
            let feed = Feed::new(price_manager, price_sender_channel, PUBLISH_INTERVAL);
            tasks.spawn(feed.run(feed_token));
        }

        drop(price_sender);

        select! {
            Some(result) = tasks.join_next() => log_task_result(result),
            _ = sigint.recv() => info!("Received interrupt signal"),
            _ = sigterm.recv() => info!("Received terminate signal"),
        }

        token.cancel();

        while let Some(result) = tasks.join_next().await {
            log_task_result(result);
        }

        Ok(())
    }
}

fn log_task_result(result: std::result::Result<Result<()>, JoinError>) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => error!(%error, "Service failed"),
        Err(error) => error!(%error, "Service task failed"),
    }
}
