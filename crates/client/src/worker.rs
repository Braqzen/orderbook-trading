use crate::{
    api::{MarketFeed, OrderBook},
    trade::Engine,
};
use eyre::Result;
use tokio::{
    select,
    signal::unix::{SignalKind, signal},
    sync::mpsc,
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct Worker {
    market: String,
    orderbook: String,
}

impl Worker {
    pub fn new(market: String, orderbook: String) -> Self {
        Self { market, orderbook }
    }

    pub async fn run(&self) -> Result<()> {
        // Handle running locally and interrupting the process with ctrl+c.
        let mut sigint = signal(SignalKind::interrupt())?;
        // Handle running in a container and terminating the process with docker stop.
        let mut sigterm = signal(SignalKind::terminate())?;

        let token = CancellationToken::new();
        let feed_token = token.child_token();
        let book_token = token.child_token();
        let engine_token = token.child_token();
        let feed_guard = token.clone().drop_guard();
        let book_guard = token.clone().drop_guard();
        let engine_guard = token.clone().drop_guard();

        let market_feed = MarketFeed::new(self.market.clone());
        let orderbook = OrderBook::new(self.orderbook.clone());
        let engine = Engine::new();
        let (feed_sender, feed_receiver) = mpsc::channel::<(String, f64)>(128);
        let (order_sender, order_receiver) = mpsc::channel(128);

        let mut tasks = JoinSet::new();

        tasks.spawn(async move {
            let _guard = engine_guard;
            engine.run(feed_receiver, order_sender, engine_token).await
        });
        tasks.spawn(async move {
            let _guard = book_guard;
            orderbook.run(order_receiver, book_token).await
        });
        tasks.spawn(async move {
            let _guard = feed_guard;
            market_feed.run(feed_sender, feed_token).await
        });

        select! {
            _ = token.cancelled() => info!("Service exited"),
            _ = sigint.recv() => info!("Received interrupt signal"),
            _ = sigterm.recv() => info!("Received terminate signal"),
        }

        token.cancel();

        while let Some(result) = tasks.join_next().await {
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => error!(%error, "Service failed"),
                Err(error) => error!(%error, "Service task failed"),
            }
        }

        Ok(())
    }
}
