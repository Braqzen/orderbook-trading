use crate::{engine::Engine, trade::Instrument, websocket::WsServer};
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

pub struct Worker {
    server: WsServer,
    engine: Engine,
}

impl Worker {
    pub fn new(ws: SocketAddr, instrument: String) -> Result<Self> {
        let instrument = Instrument::try_from(instrument.as_str())?;
        let (order_sender, order_receiver) = mpsc::channel(128);

        Ok(Self {
            server: WsServer::new(ws, order_sender),
            engine: Engine::new(instrument, order_receiver),
        })
    }

    pub async fn run(self) -> Result<()> {
        // Handle running locally and interrupting the process with ctrl+c.
        let mut sigint = signal(SignalKind::interrupt())?;
        // Handle running in a container and terminating the process with docker stop.
        let mut sigterm = signal(SignalKind::terminate())?;

        let token = CancellationToken::new();
        let ws_token = token.child_token();
        let engine_token = token.child_token();

        let mut tasks = JoinSet::new();

        tasks.spawn(self.server.run(ws_token));
        tasks.spawn(self.engine.run(engine_token));

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
