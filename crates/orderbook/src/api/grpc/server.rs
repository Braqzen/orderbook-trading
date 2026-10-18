//! The server which accepts order submissions from the orderbook gateway.
//!
//! The orderbook is the server because it is the only writer for the instrument.
//! The gateway is the client that connects and submits place and cancel requests.

use crate::{
    api::grpc::service::OrderbookService, metrics::OrderbookMetrics, proto::orderbook_server::OrderbookServer,
    trade::{Instrument, Request},
};
use eyre::Result;
use std::net::SocketAddr;
use tokio::{net::TcpListener, sync::mpsc::Sender};
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;

pub struct GrpcServer {
    /// The orderbook listens on `socket` while the gateway connects to submit orders
    socket: SocketAddr,
    instrument: Instrument,
    orders: Sender<Request>,
    metrics: OrderbookMetrics,
}

impl GrpcServer {
    pub fn new(
        socket: SocketAddr,
        instrument: Instrument,
        orders: Sender<Request>,
        metrics: OrderbookMetrics,
    ) -> Self {
        Self {
            socket,
            instrument,
            orders,
            metrics,
        }
    }

    pub async fn run(self, token: CancellationToken) -> Result<()> {
        let listener = TcpListener::bind(self.socket).await?;
        let service = OrderbookService::new(self.instrument, self.orders, self.metrics);

        Server::builder()
            .add_service(OrderbookServer::new(service))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), token.cancelled_owned())
            .await
            .map_err(Into::into)
    }
}
