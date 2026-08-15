use crate::{
    api::websocket,
    proto::{
        PriceUpdate,
        generator_feed_server::{GeneratorFeed, GeneratorFeedServer},
    },
};
use eyre::Result;
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    net::TcpListener,
    signal::unix::{SignalKind, signal},
    sync::{
        RwLock,
        broadcast::{self, Sender},
    },
    task::JoinSet,
};
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::{Request, Response, Status, Streaming, transport::Server};
use tracing::{error, info};

struct State {
    current_price: RwLock<f64>,
    broadcast: Sender<f64>,
}

pub struct Worker {
    state: Arc<State>,
    socket: SocketAddr,
    ws: SocketAddr,
}

impl Worker {
    pub fn new(socket: SocketAddr, ws: SocketAddr) -> Self {
        let (sender, _receiver) = broadcast::channel::<f64>(128);

        Self {
            state: Arc::new(State {
                current_price: RwLock::new(0.0),
                broadcast: sender,
            }),
            socket,
            ws,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Handle running locally and interrupting the process with ctrl+c.
        let mut sigint = signal(SignalKind::interrupt())?;
        // Handle running in a container and terminating the process with docker stop.
        let mut sigterm = signal(SignalKind::terminate())?;

        let listener = TcpListener::bind(self.socket.clone()).await?;

        let token = CancellationToken::new();
        let grpc_token = token.child_token();
        let ws_token = token.child_token();
        let grpc_guard = token.clone().drop_guard();
        let ws_guard = token.clone().drop_guard();

        let generator_feed = MyGeneratorFeed {
            state: Arc::clone(&self.state),
        };

        let tx = self.state.broadcast.clone();
        let ws = self.ws;
        let mut tasks = JoinSet::new();

        tasks.spawn(async move {
            let _guard = grpc_guard;

            let result: Result<()> = Server::builder()
                .add_service(GeneratorFeedServer::new(generator_feed))
                .serve_with_incoming_shutdown(
                    TcpListenerStream::new(listener),
                    grpc_token.cancelled_owned(),
                )
                .await
                .map_err(Into::into);

            result
        });

        tasks.spawn(async move {
            let _guard = ws_guard;
            websocket(tx, ws, ws_token).await
        });

        tokio::select! {
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

struct MyGeneratorFeed {
    state: Arc<State>,
}

#[tonic::async_trait]
impl GeneratorFeed for MyGeneratorFeed {
    async fn publish_price(
        &self,
        request: Request<Streaming<PriceUpdate>>,
    ) -> Result<Response<()>, Status> {
        let mut prices = request.into_inner();

        loop {
            match prices.message().await {
                Ok(Some(price)) => {
                    info!(price = price.value, "Price update");
                    *self.state.current_price.write().await = price.value;
                    let _ = self.state.broadcast.send(price.value);
                }
                Ok(None) => return Ok(Response::new(())),
                Err(error) => return Err(error),
            }
        }
    }
}
