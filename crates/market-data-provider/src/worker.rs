use crate::{
    api::WsServer,
    grpc::MarketDataProviderService,
    proto::{PriceUpdate, market_data_provider_server::MarketDataProviderServer},
    state::State,
};
use eyre::Result;
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
    sync::broadcast,
    task::{JoinError, JoinSet},
};
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;
use tracing::{error, info};

pub struct Worker {
    socket: SocketAddr,
    state: Arc<State>,
    ws: WsServer,
}

impl Worker {
    pub fn new(socket: SocketAddr, ws: SocketAddr) -> Self {
        let (price_channel, _price_receiver) = broadcast::channel::<PriceUpdate>(128);

        let ws = WsServer::new(ws, price_channel.clone());

        Self {
            socket,
            state: Arc::new(State::new(price_channel)),
            ws,
        }
    }

    pub async fn run(self) -> Result<()> {
        // Handle running locally and interrupting the process with ctrl+c.
        let mut sigint = signal(SignalKind::interrupt())?;
        // Handle running in a container and terminating the process with docker stop.
        let mut sigterm = signal(SignalKind::terminate())?;

        let listener = TcpListener::bind(self.socket).await?;

        let token = CancellationToken::new();
        let grpc_token = token.child_token();
        let ws_token = token.child_token();

        let market_data_provider = MarketDataProviderService::new(self.state);
        let mut tasks = JoinSet::new();

        tasks.spawn(async move {
            let result: Result<()> = Server::builder()
                .add_service(MarketDataProviderServer::new(market_data_provider))
                .serve_with_incoming_shutdown(
                    TcpListenerStream::new(listener),
                    grpc_token.cancelled_owned(),
                )
                .await
                .map_err(Into::into);

            result
        });

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
