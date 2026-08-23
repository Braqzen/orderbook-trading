use crate::{
    api::{ConnectionRegistry, Rejection, Response},
    metrics::OrderbookMetrics,
    trade::{Instrument, OrderBook, Request, RiskAnalyser},
};
use eyre::Result;
use tokio::{
    select,
    sync::mpsc::{Receiver, error::TrySendError},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct Engine {
    instrument: Instrument,
    book: OrderBook,
    risk: RiskAnalyser,
    order_receiver: Receiver<Request>,
    connection_registry: ConnectionRegistry,
    metrics: OrderbookMetrics,
}

impl Engine {
    pub fn new(
        instrument: Instrument,
        order_receiver: Receiver<Request>,
        connection_registry: ConnectionRegistry,
        metrics: OrderbookMetrics,
    ) -> Self {
        Self {
            instrument: instrument.clone(),
            book: OrderBook::new(),
            risk: RiskAnalyser::new(instrument),
            order_receiver,
            connection_registry,
            metrics,
        }
    }

    pub async fn run(mut self, token: CancellationToken) -> Result<()> {
        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                request = self.order_receiver.recv() => {
                    let Some(Request { instrument, price, order }) = request else {
                        error!(instrument = %self.instrument, "Engine to orderbook api channel closed");
                        break;
                    };
                    self.metrics.global_order_dequeued();

                    match self.risk.evaluate(&instrument, &order, &price) {
                        Ok(()) => {}
                        Err(reason) => {
                            warn!(
                                instrument = %self.instrument,
                                order = %order.order_id,
                                ?reason,
                                "Order rejected"
                            );

                            let client_id = order.client_id;
                            let rejection =
                                Rejection::new(instrument, price, order, reason);
                            self.send_response(client_id, Response::Rejection(rejection))
                                .await;

                            continue;
                        }
                    }

                    let result = self.book.trade(price, order.clone());

                    let remaining = result.remaining;
                    let filled_size = order.size - remaining;

                    info!(
                        instrument = %self.instrument,
                        limit_price = %price,
                        requested_size = order.size,
                        filled_size,
                        remaining,
                        trade_count = result.trades.len() / 2,
                        side = %order.side,
                        status = %result.status(),
                        client=%order.client_id,
                        order=%order.order_id,
                        "Order processed"
                    );

                    for (client_id, trade) in result.trades {
                        self.send_response(client_id, Response::Trade(trade)).await;
                    }
                }
            }
        }

        Ok(())
    }

    async fn send_response(&self, client_id: Uuid, response: Response) {
        let client = {
            let registry = self.connection_registry.read().await;
            registry.get(&client_id).cloned()
        };

        match client {
            Some(client) => match client.try_send(response) {
                Ok(()) => {}
                Err(TrySendError::Closed(_)) => {
                    warn!(instrument = %self.instrument, client = %client_id, "Client is not connected");
                }
                Err(TrySendError::Full(_)) => {
                    warn!(instrument = %self.instrument, client = %client_id, "Client outbound queue full");
                    client.disconnect();
                    self.connection_registry.write().await.remove(&client_id);
                }
            },
            None => {
                warn!(instrument = %self.instrument, client = %client_id, "Client is not connected");
            }
        }
    }
}
