use crate::proto::{
    PriceUpdate,
    generator_feed_server::{GeneratorFeed, GeneratorFeedServer},
};
use eyre::Result;
use std::{net::SocketAddr, sync::Arc};
use tokio::{
    signal::unix::{SignalKind, signal},
    sync::RwLock,
};
use tonic::{Request, Response, Status, Streaming, transport::Server};
use tracing::info;

struct State {
    current_price: RwLock<f64>,
}

pub struct Worker {
    state: Arc<State>,
    socket: SocketAddr,
}

impl Worker {
    pub fn new(socket: SocketAddr) -> Self {
        Self {
            state: Arc::new(State {
                current_price: RwLock::new(0.0),
            }),
            socket,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Handle running locally and interrupting the process with ctrl+c.
        let mut sigint = signal(SignalKind::interrupt())?;
        // Handle running in a container and terminating the process with docker stop.
        let mut sigterm = signal(SignalKind::terminate())?;

        let shutdown = async {
            tokio::select! {
                _ = sigint.recv() => info!("Received interrupt signal"),
                _ = sigterm.recv() => info!("Received terminate signal"),
            }
        };

        let generator_feed = MyGeneratorFeed {
            state: Arc::clone(&self.state),
        };

        Server::builder()
            .add_service(GeneratorFeedServer::new(generator_feed))
            .serve_with_shutdown(self.socket, shutdown)
            .await?;

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
                    *self.state.current_price.write().await = price.value;
                    println!("{}", price.value);
                }
                Ok(None) => return Ok(Response::new(())),
                Err(error) => return Err(error),
            }
        }
    }
}
