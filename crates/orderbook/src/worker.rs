use crate::{engine::Engine, websocket::WsServer};
use eyre::Result;
use std::net::SocketAddr;
use tokio::{
    select,
    signal::unix::{SignalKind, signal},
    sync::mpsc,
    task::JoinSet,
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct Worker {
    ws: SocketAddr,
}

impl Worker {
    pub fn new(ws: SocketAddr) -> Self {
        Self { ws }
    }

    pub async fn run(&self, instrument: String) -> Result<()> {
        // Handle running locally and interrupting the process with ctrl+c.
        let mut sigint = signal(SignalKind::interrupt())?;
        // Handle running in a container and terminating the process with docker stop.
        let mut sigterm = signal(SignalKind::terminate())?;

        let token = CancellationToken::new();
        let ws_token = token.child_token();
        let engine_token = token.child_token();
        let ws_guard = token.clone().drop_guard();
        let engine_guard = token.clone().drop_guard();

        let (order_sender, order_receiver) = mpsc::channel(128);
        let ws_server = WsServer::new(self.ws, order_sender);
        let mut engine = Engine::new(instrument);

        let mut tasks = JoinSet::new();

        tasks.spawn(async move {
            let _guard = ws_guard;
            ws_server.run(ws_token).await
        });
        tasks.spawn(async move {
            let _guard = engine_guard;
            engine.run(order_receiver, engine_token).await
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
