use crate::{
    api::{
        CancelRejection, CancelRejectionReason, Cancelled, ConnectionRegistry, OrderAccepted,
        OrderRejection, Response,
    },
    metrics::OrderbookMetrics,
    trade::{Instrument, LimitOrder, OrderBook, OrderType, Price, Quantity, Request, RiskAnalyser},
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
                    let Some(request) = request else {
                        error!(instrument = %self.instrument, "Engine to orderbook api channel closed");
                        break;
                    };
                    self.metrics.global_order_dequeued();

                    match request {
                        Request::Place { instrument, price, order } => {
                            self.handle_place(instrument, price, order).await;
                        }
                        Request::Cancel { client_id, order_id, price, side } => {
                            self.handle_cancel(client_id, order_id, price, side).await;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    async fn handle_place(&mut self, instrument: Instrument, price: Price, order: LimitOrder) {
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
                let rejection = OrderRejection::new(instrument, price, order, reason);
                self.send_response(client_id, Response::OrderRejected(rejection))
                    .await;

                return;
            }
        }

        let result = self.book.trade(price, order.clone());
        let trade_count = (result.trades.len() / 2) as u64;
        if let Err(error) = self
            .metrics
            .record_orderbook(&self.book, trade_count, result.filled)
        {
            warn!(
                instrument = %self.instrument,
                order = %order.order_id,
                %error,
                "Failed to record orderbook metrics"
            );
        }

        let remaining = result.remaining;
        info!(
            instrument = %self.instrument,
            limit_price = %price,
            requested_size = %order.size,
            filled_size = %result.filled,
            remaining = %remaining,
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

        if Quantity::ZERO < remaining {
            self.send_response(
                order.client_id,
                Response::OrderAccepted(OrderAccepted {
                    order_id: order.order_id,
                }),
            )
            .await;
        }
    }

    async fn handle_cancel(
        &mut self,
        client_id: Uuid,
        order_id: Uuid,
        price: Price,
        side: OrderType,
    ) {
        let cancelled = self.book.cancel(client_id, order_id, price, side);

        if cancelled {
            info!(
                instrument = %self.instrument,
                client = %client_id,
                order = %order_id,
                price = %price,
                side = %side,
                "Order cancelled"
            );
            self.send_response(client_id, Response::Cancelled(Cancelled { order_id }))
                .await;
        } else {
            warn!(
                instrument = %self.instrument,
                client = %client_id,
                order = %order_id,
                price = %price,
                side = %side,
                "Cancel rejected"
            );
            self.send_response(
                client_id,
                Response::CancelRejected(CancelRejection {
                    order_id,
                    reason: CancelRejectionReason::OrderNotFound,
                }),
            )
            .await;
        }
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
