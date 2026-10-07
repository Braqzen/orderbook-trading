//! Creates and runs the order book system for one instrument.
//! It owns the websocket server and engine, keeping both running until shutdown.

use crate::{
    api::{SessionStore, WsServer},
    engine::Engine,
    metrics::OrderbookMetrics,
    trade::Instrument,
};
use eyre::Result;
use std::net::SocketAddr;
use tokio::{
    select,
    signal::unix::{SignalKind, signal},
    sync::mpsc,
    task::{JoinError, JoinSet},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

// TODO: how many orders in channel as buffer?
//       if this global channel is full, and some clients keep sending and fill their queues
//       their connections will be killed.
/// Number of orders to buffer across all users
const GLOBAL_ORDER_QUEUE: usize = 8192;

pub struct Worker {
    /// The only instrument this orderbook processes
    instrument: Instrument,
    /// Handles websocket connections from clients
    server: WsServer,
    /// Core system handling user requests
    engine: Engine,
}

impl Worker {
    pub fn new(ws: SocketAddr, instrument: String) -> Result<Self> {
        let instrument = Instrument::try_from(instrument.as_str())?;
        let (order_sender, order_receiver) = mpsc::channel(GLOBAL_ORDER_QUEUE);
        let sessions = SessionStore::new();
        let metrics = OrderbookMetrics::new(&instrument);

        Ok(Self {
            instrument: instrument.clone(),
            server: WsServer::new(
                ws,
                instrument.clone(),
                order_sender,
                sessions.clone(),
                metrics.clone(),
            ),
            engine: Engine::new(instrument, order_receiver, sessions, metrics),
        })
    }

    pub async fn run(self) -> Result<()> {
        // Handle running locally and interrupting the process with ctrl+c.
        let mut sigint = signal(SignalKind::interrupt())?;
        // Handle running in a container and terminating the process with docker stop.
        let mut sigterm = signal(SignalKind::terminate())?;

        // Tokens are used to cancel all tasks
        let token = CancellationToken::new();
        let ws_token = token.child_token();
        let engine_token = token.child_token();

        let mut tasks = JoinSet::new();

        tasks.spawn(self.server.run(ws_token));
        tasks.spawn(self.engine.run(engine_token));

        // If any task shuts down or a signal is received shut everything down
        select! {
            Some(result) = tasks.join_next() => log_task_result(&self.instrument, result),
            _ = sigint.recv() => info!(instrument = %self.instrument, "Received interrupt signal"),
            _ = sigterm.recv() => info!(instrument = %self.instrument, "Received terminate signal"),
        }

        token.cancel();

        while let Some(result) = tasks.join_next().await {
            log_task_result(&self.instrument, result);
        }

        Ok(())
    }
}

fn log_task_result(instrument: &Instrument, result: std::result::Result<Result<()>, JoinError>) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => error!(%instrument, %error, "Service failed"),
        Err(error) => error!(%instrument, %error, "Service task failed"),
    }
}
