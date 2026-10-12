//! The server which receives gRPC instrument/price updates from the generator
//!
//! This is a wrapper to handle the external setup while the [`PriceAggregatorService`] is our business logic.
//!
//! This architecture is actually incorrect, the aggregator ought to act as the client of the generator
//! because it acts as a middleman aggregator that collects prices from potentially multiple sources
//! i.e. multiple generators however here we use 1 generator to send many instruments.
//!
//! TODO: change the aggregator to act as the client of the generator, but maintain a single generator instead
//!       of splitting into 1 generator per instrument

use crate::{
    api::grpc::service::PriceAggregatorService,
    proto::price_aggregator_server::PriceAggregatorServer, valkey::Cache,
};
use eyre::Result;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;

pub struct GrpcServer {
    /// The aggregator is the server running on `socket` while the generator is the client
    /// which connects to send price updates
    socket: SocketAddr,

    /// The cache manages interaction with Valkey i.e. setting price updates
    cache: Cache,
}

impl GrpcServer {
    pub fn new(socket: SocketAddr, cache: Cache) -> Self {
        Self { socket, cache }
    }

    pub async fn run(self, token: CancellationToken) -> Result<()> {
        // Bind to socket that receives gRPC price updates
        let listener = TcpListener::bind(self.socket).await?;

        // Wrapper type required to impl a trait from tonic
        let price_aggregator = PriceAggregatorService::new(self.cache);

        Server::builder()
            .add_service(PriceAggregatorServer::new(price_aggregator))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), token.cancelled_owned())
            .await
            .map_err(Into::into)
    }
}
