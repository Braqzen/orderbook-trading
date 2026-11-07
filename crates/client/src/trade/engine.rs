//! Uses the [`Trader`] to handle trading based on price updates and orderbook responses

use crate::{
    api::{
        ExecutionReport, MarketPrice, OrderBookRegistry, RequestMetadata, Response, SubmitOutcome,
    },
    metrics::ClientMetrics,
    trade::{TradeAction, Trader},
    valkey::SequenceCache,
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
    /// Used for attaching to logs
    id: Uuid,
    /// Actor deciding trade actions
    trader: Trader,
    /// Receive price updates from the previous stage
    price_receiver_channel: Receiver<MarketPrice>,
    /// Await responses from the orderbook
    response_receiver_channel: Receiver<ExecutionReport>,
    /// Learns whether a place request reached the orderbook websocket
    submit_outcome_receiver_channel: Receiver<SubmitOutcome>,
    /// Tracks metrics
    metrics: ClientMetrics,
    /// Stores the last applied execution sequence
    sequences: SequenceCache,
    /// Routes orders to logged-in orderbook tasks
    orderbook_registry: OrderBookRegistry,
}

impl Engine {
    pub fn new(
        id: Uuid,
        trader: Trader,
        price_receiver_channel: Receiver<MarketPrice>,
        response_receiver_channel: Receiver<ExecutionReport>,
        submit_outcome_receiver_channel: Receiver<SubmitOutcome>,
        metrics: ClientMetrics,
        sequences: SequenceCache,
        orderbook_registry: OrderBookRegistry,
    ) -> Self {
        Self {
            id,
            trader,
            price_receiver_channel,
            response_receiver_channel,
            submit_outcome_receiver_channel,
            metrics,
            sequences,
            orderbook_registry,
        }
    }

    pub async fn run(mut self, token: CancellationToken) -> Result<()> {
        self.trader.record_inventory();

        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                // The market data gateway has emitted a new price update, evaluate price for trade action
                price = self.price_receiver_channel.recv() => {
                    let Some(price) = price else {
                        error!(client = %self.id, "Market data gateway channel closed");
                        break;
                    };

                    if !self.orderbook_registry.connected(&price.instrument).await {
                        continue;
                    }

                    match self.trader.select_action(price) {
                        TradeAction::Skip => {}
                        TradeAction::Place {
                            instrument,
                            price: value,
                            size,
                            side,
                        } => {
                            let Some(order) = self
                                .trader
                                .reserve(self.id, instrument.clone(), value, size, side)
                            else {
                                continue;
                            };

                            if !self.route_request(RequestMetadata::place(&order)).await {
                                self.trader.rollback(&order);
                            }
                        }
                        TradeAction::Cancel { order_id } => {
                            let Some(order) = self.trader.opened_order(order_id) else {
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

                            self.route_request(RequestMetadata::cancel(&order)).await;
                        }
                    }
                }

                // Handle whether we sent request through orderbook ws connection
                outcome = self.submit_outcome_receiver_channel.recv() => {
                    let Some(outcome) = outcome else {
                        error!(client = %self.id, "Submit outcome channel closed");
                        break;
                    };

                    if outcome.submitted {
                        self.trader.commit(outcome.order);
                    } else {
                        self.trader.rollback(&outcome.order);
                    }
                }

                // Process the response of the orderbook service
                response = self.response_receiver_channel.recv() => {
                    let Some(report) = response else {
                        error!(client = %self.id, "Orderbook response channel closed");
                        break;
                    };

                    if self.already_applied(&report).await {
                        continue;
                    }

                    let exec_seq = report.response.exec_seq();
                    let instrument = report.instrument.clone();

                    // TODO: split accepted into untraded but in book and partially unfilled rest remains in book
                    match report.response {
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

                    if exec_seq != 0
                        && let Err(error) = self.sequences.store(self.id, &instrument, exec_seq).await
                    {
                        error!(client = %self.id, %instrument, %exec_seq, %error, "Failed to store execution sequence");
                    }
                }
            }
        }

        Ok(())
    }

    /// Forward the request from the engine to 1 of the orderbook tasks (1 task per instrument)
    async fn route_request(&mut self, request: RequestMetadata) -> bool {
        let instrument = request.order.instrument.clone();
        let Some(order_sender) = self.orderbook_registry.sender(&instrument).await else {
            warn!(client = %self.id, %instrument, "No logged-in orderbook for instrument");
            return false;
        };

        match order_sender.try_send(request) {
            Ok(()) => true,
            Err(TrySendError::Closed(_)) => {
                self.orderbook_registry
                    .remove(&instrument, &order_sender)
                    .await;

                error!(client = %self.id, %instrument, "Orderbook order channel closed");
                false
            }
            Err(TrySendError::Full(_)) => {
                warn!(client = %self.id, %instrument, "Orderbook order queue full");
                false
            }
        }
    }

    // Execution 5 before 4 advances the cursor and drops 4, desyncing the client from the log.
    // This demo avoids that: one client key is written in order on one partition.
    // Production would request 4 before applying 5.
    async fn already_applied(&mut self, report: &ExecutionReport) -> bool {
        let exec_seq = report.response.exec_seq();

        if exec_seq == 0 {
            return false;
        }

        match self
            .sequences
            .last_exec_seq(self.id, &report.instrument)
            .await
        {
            Ok(last_exec_seq) => exec_seq <= last_exec_seq,
            Err(error) => {
                error!(client = %self.id, instrument = %report.instrument, %error, "Failed to read execution sequence");
                true
            }
        }
    }
}
