use crate::{
    api::{MarketFeed, MarketPrice, OrderBook},
    trade::{Engine, Inventory},
};
use eyre::{Result, eyre};
use tokio::{
    select,
    signal::unix::{SignalKind, signal},
    sync::mpsc,
    task::{JoinError, JoinSet},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct Worker {
    market_feed: MarketFeed,
    orderbook: OrderBook,
    engine: Engine,
}

impl Worker {
    pub fn new(market: String, orderbook: String, inventory: String) -> Result<Self> {
        let values = inventory
            .split(',')
            .map(|asset| {
                let (name, amount) = asset
                    .split_once(':')
                    .ok_or_else(|| eyre!("Invalid inventory entry: {asset}"))?;
                let amount = amount
                    .parse::<f64>()
                    .map_err(|error| eyre!("Invalid amount for {name}: {error}"))?;
                Ok((name.to_owned(), amount))
            })
            .collect::<Result<Vec<_>>>()?;

        let market_feed = MarketFeed::new(market);
        let orderbook = OrderBook::new(orderbook);
        let engine = Engine::new(Inventory::new(values));

        Ok(Self {
            market_feed,
            orderbook,
            engine,
        })
    }

    pub async fn run(self) -> Result<()> {
        // Handle running locally and interrupting the process with ctrl+c.
        let mut sigint = signal(SignalKind::interrupt())?;
        // Handle running in a container and terminating the process with docker stop.
        let mut sigterm = signal(SignalKind::terminate())?;

        let token = CancellationToken::new();
        let feed_token = token.child_token();
        let book_token = token.child_token();
        let engine_token = token.child_token();

        let (feed_sender, feed_receiver) = mpsc::channel::<MarketPrice>(128);
        let (order_sender, order_receiver) = mpsc::channel(128);

        let mut tasks = JoinSet::new();

        tasks.spawn(self.engine.run(feed_receiver, order_sender, engine_token));
        tasks.spawn(self.orderbook.run(order_receiver, book_token));
        tasks.spawn(self.market_feed.run(feed_sender, feed_token));

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
