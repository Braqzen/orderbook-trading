use crate::{
    api::grpc::service::MarketDataProviderService,
    proto::{PriceUpdate, market_data_provider_server::MarketDataProviderServer},
};
use eyre::Result;
use std::net::SocketAddr;
use tokio::{net::TcpListener, sync::broadcast::Sender};
use tokio_stream::wrappers::TcpListenerStream;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;

pub struct GrpcServer {
    /// Socket to run the server on
    socket: SocketAddr,

    price_sender_channel: Sender<PriceUpdate>,
}

impl GrpcServer {
    pub fn new(socket: SocketAddr, price_sender_channel: Sender<PriceUpdate>) -> Self {
        Self {
            socket,
            price_sender_channel,
        }
    }

    pub async fn run(self, token: CancellationToken) -> Result<()> {
        // Bind to socket that receives gRPC price updates
        let listener = TcpListener::bind(self.socket).await?;
        let market_data_provider = MarketDataProviderService::new(self.price_sender_channel);

        Server::builder()
            .add_service(MarketDataProviderServer::new(market_data_provider))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), token.cancelled_owned())
            .await
            .map_err(Into::into)
    }
}
