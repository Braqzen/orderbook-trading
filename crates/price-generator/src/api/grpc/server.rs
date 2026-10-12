//! The server which streams gRPC instrument/price updates to the aggregator.
//!
//! This is a wrapper to handle the external setup while the [`PriceGeneratorService`] is our business logic.
//!
//! The generator is the server because it represents a source of prices. The aggregator is the client
//! that connects, ingests, and processes the feed. One generator sends many instruments.

use crate::{
    api::grpc::service::PriceGeneratorService, proto::PriceUpdate,
    proto::price_generator_server::PriceGeneratorServer,
};
use eyre::Result;
use std::net::SocketAddr;
use tokio::{net::TcpListener, sync::broadcast::Sender};
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;

pub struct GrpcServer {
    /// The generator is the server running on `socket` while the aggregator is the client
    /// which connects to receive price updates
    socket: SocketAddr,

    /// Generated prices streamed to each connected aggregator
    prices: Sender<PriceUpdate>,
}

impl GrpcServer {
    pub fn new(socket: SocketAddr, prices: Sender<PriceUpdate>) -> Self {
        Self { socket, prices }
    }

    pub async fn run(self, token: CancellationToken) -> Result<()> {
        // Bind to socket that streams gRPC price updates
        let listener = TcpListener::bind(self.socket).await?;

        // Wrapper type required to impl a trait from tonic
        let price_generator = PriceGeneratorService::new(self.prices);

        Server::builder()
            .add_service(PriceGeneratorServer::new(price_generator))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), token.cancelled_owned())
            .await
            .map_err(Into::into)
    }
}
