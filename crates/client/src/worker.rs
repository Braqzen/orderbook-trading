use crate::{
    api::{MarketFeed, MarketPrice, OrderBook},
    config::Config,
    randomiser::Randomiser,
    trade::Engine,
};
use eyre::Result;
use tokio::{
    select,
    signal::unix::{SignalKind, signal},
    sync::mpsc,
    task::{JoinError, JoinSet},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};
use uuid::Uuid;

pub struct Worker {
    client_id: Uuid,
    market_feed: MarketFeed,
    orderbook: OrderBook,
    engine: Engine,
}

impl Worker {
    pub fn new(client_id: Uuid, market: String, config: Config) -> Result<Self> {
        let instrument_urls = config.instruments.clone();
        let randomiser = Randomiser::new(config)?;
        let inventory = randomiser.inventory()?;
        let instruments = randomiser.instruments();

        let (price_sender_channel, price_receiver_channel) = mpsc::channel::<MarketPrice>(128);
        let (order_sender_channel, order_receiver_channel) = mpsc::channel(128);
        let (response_sender_channel, response_receiver_channel) = mpsc::channel(128);

        let market_feed =
            MarketFeed::new(client_id, market, instruments.clone(), price_sender_channel);
        let orderbook = OrderBook::new(
            client_id,
            instruments,
            instrument_urls,
            order_receiver_channel,
            response_sender_channel,
        )?;
        let engine = Engine::new(
            client_id,
            inventory,
            price_receiver_channel,
            order_sender_channel,
            response_receiver_channel,
        );

        Ok(Self {
            client_id,
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

        let mut tasks = JoinSet::new();

        tasks.spawn(self.engine.run(engine_token));
        tasks.spawn(self.orderbook.run(book_token));
        tasks.spawn(self.market_feed.run(feed_token));

        select! {
            Some(result) = tasks.join_next() => log_task_result(&self.client_id, result),
            _ = sigint.recv() => info!(client = %self.client_id, "Received interrupt signal"),
            _ = sigterm.recv() => info!(client = %self.client_id, "Received terminate signal"),
        }

        token.cancel();

        while let Some(result) = tasks.join_next().await {
            log_task_result(&self.client_id, result);
        }

        Ok(())
    }
}

fn log_task_result(client_id: &Uuid, result: std::result::Result<Result<()>, JoinError>) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => error!(client = %client_id, %error, "Service failed"),
        Err(error) => error!(client = %client_id, %error, "Service task failed"),
    }
}
