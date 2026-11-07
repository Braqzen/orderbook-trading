//! Top-level client process: wires channels, builds the engine and market data gateway, and
//! spawns one orderbook task per instrument. `run` supervises those tasks until shutdown, a core
//! task failure, or every orderbook task has ended.

use crate::{
    api::{
        ExecutionReport, MarketDataGateway, MarketPrice, OrderBook, OrderBookRegistry,
        SubmitOutcome, WsUrl,
    },
    config::Config,
    metrics::ClientMetrics,
    randomiser::Randomiser,
    trade::{Engine, Trader},
    valkey::SequenceCache,
};
use eyre::{Result, ensure, eyre};
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
    market_data_gateway: MarketDataGateway,
    /// Each orderbook is spawned as its own task to separate traffic from different instruments
    orderbooks: Vec<OrderBook>,
}

impl Worker {
    pub fn new(
        market_data_gateway_url: WsUrl,
        config: Config,
        sequences: SequenceCache,
    ) -> Result<Self> {
        // Project spawns multiple clients with random state so use an ID for logs, metrics.
        let client_id = Uuid::new_v4();
        let metrics = ClientMetrics::new(client_id);

        // Associates an instrument to the orderbook to send to
        let orderbook_urls = config.instruments.clone();

        let trade_limits = config.trade_limits.clone();

        // Load the same config for all clients then randomise entries for simulation
        let randomiser = Randomiser::new(config)?;
        let (instruments, inventory) = randomiser.randomise()?;
        ensure!(
            !instruments.is_empty(),
            "Must subscribe to at least one instrument with an orderbook"
        );

        // Price updates should only reach the engine if the client logged into the orderbook
        let orderbook_registry = OrderBookRegistry::new();

        // Given new price events and client state decide on next trade action
        let trader = Trader::new(trade_limits, inventory, metrics.clone());

        // First, receive price updates from the market data gateway and send them to the engine for evaluation
        let (price_sender_channel, price_receiver_channel) = mpsc::channel::<MarketPrice>(128);
        // Later, when the orderbook receives a response from the service, forward it to engine for processing
        let (response_sender_channel, response_receiver_channel) =
            mpsc::channel::<ExecutionReport>(128);

        // Used when the orderbook api confirms a place request reached the gateway websocket
        // Internally we reserve an asset making it unusable and then attempt to send to orderbook which can fail
        // Communicate the outcome of the send to undo reservation or mark as an open order
        let (submit_outcome_sender_channel, submit_outcome_receiver_channel) =
            mpsc::channel::<SubmitOutcome>(128);

        // First step of New Price Update Event -> Send to Engine for processing
        let market_data_gateway = MarketDataGateway::new(
            client_id,
            market_data_gateway_url,
            instruments.clone(),
            price_sender_channel,
            orderbook_registry.clone(),
            metrics.clone(),
        );
        // Engine decides if it wants to trade, sends actions to orderbook
        let engine = Engine::new(
            client_id,
            trader,
            price_receiver_channel,
            response_receiver_channel,
            submit_outcome_receiver_channel,
            metrics,
            sequences.clone(),
            orderbook_registry.clone(),
        );

        // Lastly each orderbook sends actions for one instrument then forwards responses to engine
        let mut orderbooks = Vec::with_capacity(instruments.len());
        for instrument in instruments {
            let url = orderbook_urls.get(&instrument).cloned().ok_or_else(|| {
                eyre!("Missing orderbook URL for subscribed instrument {instrument}")
            })?;

            orderbooks.push(OrderBook::new(
                client_id,
                instrument,
                url,
                response_sender_channel.clone(),
                submit_outcome_sender_channel.clone(),
                sequences.clone(),
                orderbook_registry.clone(),
            ));
        }

        Ok(Self {
            client_id,
            market_data_gateway,
            orderbooks,
            engine,
        })
    }

    pub async fn run(self) -> Result<()> {
        // Handle running locally and interrupting the process with ctrl+c.
        let mut sigint = signal(SignalKind::interrupt())?;
        // Handle running in a container and terminating the process with docker stop.
        let mut sigterm = signal(SignalKind::terminate())?;

        // TODO: no graceful clean up, nuke it all, need to fix or inventory will corrupt
        let shutdown_token = CancellationToken::new();

        // Separate core tasks that must continue to function from independent books where any
        // 1 book may fail and the rest of the system ought to continue to function
        let mut core_tasks = JoinSet::new();
        let mut orderbook_tasks = JoinSet::new();

        core_tasks.spawn(self.engine.run(shutdown_token.clone()));
        core_tasks.spawn(self.market_data_gateway.run(shutdown_token.clone()));
        for orderbook in self.orderbooks {
            orderbook_tasks.spawn(orderbook.run(shutdown_token.child_token()));
        }

        loop {
            select! {
                Some(result) = core_tasks.join_next() => {
                    log_task_result(&self.client_id, result);
                    break;
                }
                Some(result) = orderbook_tasks.join_next(), if !orderbook_tasks.is_empty() => {
                    log_task_result(&self.client_id, result);
                    if orderbook_tasks.is_empty() {
                        break;
                    }
                }
                _ = sigint.recv() => {
                    info!(client = %self.client_id, "Received interrupt signal");
                    break;
                }
                _ = sigterm.recv() => {
                    info!(client = %self.client_id, "Received terminate signal");
                    break;
                }
            }
        }

        shutdown_token.cancel();

        while let Some(result) = core_tasks.join_next().await {
            log_task_result(&self.client_id, result);
        }
        while let Some(result) = orderbook_tasks.join_next().await {
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
