use crate::{
    api::{GrpcServer, WsServer},
    proto::PriceUpdate,
};
use eyre::Result;
use std::net::SocketAddr;
use tokio::{
    signal::unix::{SignalKind, signal},
    sync::broadcast,
    task::{JoinError, JoinSet},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info};

pub struct Worker {
    grpc: GrpcServer,
    ws: WsServer,
}

impl Worker {
    pub fn new(socket: SocketAddr, ws: SocketAddr) -> Self {
        // Create 1 broadcast channel to send price updates through
        // Each connected client subscribes to get their own receiver
        let (price_sender_channel, _) = broadcast::channel::<PriceUpdate>(128);

        // Each connection subscribes and gets its own receiver channel from the sender
        let ws = WsServer::new(ws, price_sender_channel.clone());

        // While the sender exists in gRPC to push updates to all subscribed clients
        let grpc = GrpcServer::new(socket, price_sender_channel);

        Self { grpc, ws }
    }

    pub async fn run(self) -> Result<()> {
        // Handle running locally and interrupting the process with ctrl+c.
        let mut sigint = signal(SignalKind::interrupt())?;
        // Handle running in a container and terminating the process with docker stop.
        let mut sigterm = signal(SignalKind::terminate())?;

        let token = CancellationToken::new();
        let grpc_token = token.child_token();
        let ws_token = token.child_token();

        let mut tasks = JoinSet::new();

        tasks.spawn(self.grpc.run(grpc_token));
        tasks.spawn(self.ws.run(ws_token));

        tokio::select! {
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
