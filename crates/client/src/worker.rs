use crate::{
    api::{MarketDataProvider, MarketPrice, OrderBook, WsUrl},
    config::Config,
    metrics::ClientMetrics,
    randomiser::Randomiser,
    trade::{Engine, Trader},
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
    /// Orchestrates trade actions and accounting
    engine: Engine,
    /// Takes market price events and forwards to engine for processing
    market_data_provider: MarketDataProvider,
    /// Takes trade actions from engine to send to book and forward responses back to engine
    orderbook: OrderBook,
}

impl Worker {
    pub fn new(market_data_provider_url: WsUrl, config: Config) -> Result<Self> {
        // Project spawns multiple clients with random state so use an ID for logs, metrics.
        let client_id = Uuid::new_v4();
        let metrics = ClientMetrics::new(client_id);

        // Associates an instrument to the orderbook to send to
        let orderbook_urls = config.instruments.clone();

        let trade_limits = config.trade_limits.clone();

        // Load the same config for all clients then randomise entries for simulation
        let randomiser = Randomiser::new(config)?;
        let (instruments, inventory) = randomiser.randomise()?;

        // Given new price events and client state decide on next trade action
        let trader = Trader::new(trade_limits, inventory, metrics.clone());

        // Used to send price updates from the market data provider to the engine for evaluation
        let (price_sender_channel, price_receiver_channel) = mpsc::channel::<MarketPrice>(128);
        // Used to send orders from engine to orderbook api
        let (order_sender_channel, order_receiver_channel) = mpsc::channel(128);
        // Used to forward orderbook responses back to engine
        let (response_sender_channel, response_receiver_channel) = mpsc::channel(128);

        // First step of New Price Update Event -> Send to Engine for processing
        let market_data_provider = MarketDataProvider::new(
            client_id,
            market_data_provider_url,
            instruments.clone(),
            price_sender_channel,
            metrics.clone(),
        );
        // Engine decides if it wants to trade, sends actions to orderbook
        let engine = Engine::new(
            client_id,
            trader,
            price_receiver_channel,
            order_sender_channel,
            response_receiver_channel,
            metrics,
        );
        // Lastly orderbook sends actions to orderbook service then forwards responses to engine
        let orderbook = OrderBook::new(
            client_id,
            instruments,
            orderbook_urls,
            order_receiver_channel,
            response_sender_channel,
        )?;

        Ok(Self {
            client_id,
            market_data_provider,
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
        let provider_token = token.child_token();
        let book_token = token.child_token();
        let engine_token = token.child_token();

        let mut tasks = JoinSet::new();

        tasks.spawn(self.engine.run(engine_token));
        tasks.spawn(self.orderbook.run(book_token));
        tasks.spawn(self.market_data_provider.run(provider_token));

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
