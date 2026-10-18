//! Creates and runs the order book system for one instrument.
//! It owns the gRPC server, engine, and execution publisher, keeping them running until shutdown.

use crate::{
    api::GrpcServer,
    engine::Engine,
    environment::Environment,
    execution::Publisher,
    metrics::OrderbookMetrics,
    trade::Instrument,
};
use eyre::Result;
use std::{net::SocketAddr, str::FromStr};
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
/// Number of executions waiting to be written to Redpanda
const PUBLISH_QUEUE: usize = 8192;

pub struct Worker {
    /// The only instrument this orderbook processes
    instrument: Instrument,
    /// Accepts order submissions from the gateway
    server: GrpcServer,
    /// Core system handling user requests
    engine: Engine,
    /// Writes executions to Redpanda
    publisher: Publisher,
}

impl Worker {
    pub fn new(environment: Environment) -> Result<Self> {
        let instrument = Instrument::try_from(environment.instrument.as_str())?;
        let grpc = SocketAddr::from_str(&environment.grpc)?;
        let (order_sender, order_receiver) = mpsc::channel(GLOBAL_ORDER_QUEUE);
        let (execution_sender, execution_receiver) = mpsc::channel(PUBLISH_QUEUE);
        let metrics = OrderbookMetrics::new(&instrument);
        let producer = Publisher::producer(&environment.redpanda_brokers)?;

        Ok(Self {
            instrument: instrument.clone(),
            server: GrpcServer::new(grpc, instrument.clone(), order_sender, metrics.clone()),
            engine: Engine::new(instrument.clone(), order_receiver, execution_sender, metrics),
            publisher: Publisher::new(instrument, producer, execution_receiver),
        })
    }

    pub async fn run(self) -> Result<()> {
        // Handle running locally and interrupting the process with ctrl+c.
        let mut sigint = signal(SignalKind::interrupt())?;
        // Handle running in a container and terminating the process with docker stop.
        let mut sigterm = signal(SignalKind::terminate())?;

        // Tokens are used to cancel all tasks
        let token = CancellationToken::new();
        let grpc_token = token.child_token();
        let engine_token = token.child_token();
        let publisher_token = token.child_token();

        let mut tasks = JoinSet::new();

        tasks.spawn(self.server.run(grpc_token));
        tasks.spawn(self.engine.run(engine_token));
        tasks.spawn(self.publisher.run(publisher_token));

        // If any task shuts down or a signal is received shut everything down
        select! {
            Some(result) = tasks.join_next() => Self::log_task_result(&self.instrument, result),
            _ = sigint.recv() => info!(instrument = %self.instrument, "Received interrupt signal"),
            _ = sigterm.recv() => info!(instrument = %self.instrument, "Received terminate signal"),
        }

        token.cancel();

        while let Some(result) = tasks.join_next().await {
            Self::log_task_result(&self.instrument, result);
        }

        Ok(())
    }

    fn log_task_result(instrument: &Instrument, result: std::result::Result<Result<()>, JoinError>) {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(error)) => error!(%instrument, %error, "Service failed"),
            Err(error) => error!(%instrument, %error, "Service task failed"),
        }
    }
}
