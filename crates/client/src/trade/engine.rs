use crate::{
    api::{MarketPrice, Request, RequestMetadata, Response},
    metrics::ClientMetrics,
    trade::{TradeAction, Trader},
};
use eyre::Result;
use tokio::{
    select,
    sync::mpsc::{Receiver, Sender},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct Engine {
    /// Used for attaching to logs
    id: Uuid,
    /// Actor deciding trade actions
    trader: Trader,
    /// Receive price updates from the previous stage
    price_receiver_channel: Receiver<MarketPrice>,
    /// Forward orders to the next stage which sends them to the orderbook
    order_sender_channel: Sender<RequestMetadata>,
    /// Await responses from the orderbook
    response_receiver_channel: Receiver<Response>,
    /// Tracks metrics
    metrics: ClientMetrics,
}

impl Engine {
    pub fn new(
        id: Uuid,
        trader: Trader,
        price_receiver_channel: Receiver<MarketPrice>,
        order_sender_channel: Sender<RequestMetadata>,
        response_receiver_channel: Receiver<Response>,
        metrics: ClientMetrics,
    ) -> Self {
        Self {
            id,
            trader,
            price_receiver_channel,
            order_sender_channel,
            response_receiver_channel,
            metrics,
        }
    }

    pub async fn run(mut self, token: CancellationToken) -> Result<()> {
        self.trader.record_inventory();

        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                response = self.response_receiver_channel.recv() => {
                    let Some(response) = response else {
                        error!(client = %self.id, "Orderbook response channel closed");
                        break;
                    };

                    // TODO: split accepted into untraded but in book and partially unfilled rest remains in book
                    match response {
                        Response::Trade(trade) => {
                            self.metrics.record_orderbook_response("trade");
                            self.trader.apply_trade(self.id, trade);
                        }
                        Response::OrderAccepted(accepted) => {
                            self.metrics.record_orderbook_response("order_accepted");
                            info!(client = %self.id, order = %accepted.order_id, "Order accepted");
                        }
                        Response::OrderRejected(rejection) => {
                            self.metrics.record_orderbook_response("order_rejected");
                            self.trader.apply_order_rejection(self.id, rejection);
                        }
                        Response::Cancelled(cancelled) => {
                            self.metrics.record_orderbook_response("cancelled");
                            self.trader.apply_cancelled(self.id, cancelled);
                        }
                        Response::CancelRejected(rejection) => {
                            self.metrics.record_orderbook_response("cancel_rejected");
                            warn!(client = %self.id, order = %rejection.order_id, ?rejection.reason, "Cancel rejection received");
                        }
                    }
                }

                price = self.price_receiver_channel.recv() => {
                    let Some(price) = price else {
                        error!(client = %self.id, "Market data provider channel closed");
                        break;
                    };

                    match self.trader.process_event(price) {
                        TradeAction::Skip => {}
                        TradeAction::Place {
                            instrument,
                            price: value,
                            size,
                            side,
                        } => {
                            let Some(order) = self
                                .trader
                                .prepare_place(self.id, instrument.clone(), value, size, side)
                            else {
                                continue;
                            };

                            let request = RequestMetadata {
                                instrument: instrument.clone(),
                                message: Request::place(order.clone()),
                            };

                            if self.order_sender_channel.send(request).await.is_err() {
                                self.trader.rollback_place(&order);
                                error!(client = %self.id, "Order channel closed");
                                break;
                            }

                            self.trader.confirm_place(order);
                        }
                        TradeAction::Cancel { order_id } => {
                            let Some(order) = self.trader.order_for_cancel(order_id) else {
                                warn!(client = %self.id, order = %order_id, "Cancel requested for unknown order");
                                continue;
                            };

                            info!(
                                client = %self.id,
                                order = %order_id,
                                instrument = %order.instrument,
                                price = %order.price,
                                side = %order.side,
                                "Sending cancel"
                            );

                            let request = RequestMetadata {
                                instrument: order.instrument.clone(),
                                message: Request::cancel(self.id, order_id, order.price, order.side),
                            };

                            if self.order_sender_channel.send(request).await.is_err() {
                                error!(client = %self.id, order = %order_id, "Order channel closed");
                                break;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
