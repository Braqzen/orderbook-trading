//! Runs the order book for one instrument as its sole writer, processing requests sequentially.
//! It validates and matches orders, applies cancellations, and publishes an execution for each client.

use crate::{
    api::{
        CancelRejection, CancelRejectionReason, Cancelled, OrderAccepted, OrderRejection, Response,
        Trade,
    },
    execution::{ExecutionSequence, OutboundExecution},
    metrics::OrderbookMetrics,
    trade::{Instrument, LimitOrder, OrderBook, OrderType, Price, Quantity, Request, RiskAnalyser},
};
use eyre::{Result, eyre};
use tokio::{select, sync::mpsc::{Receiver, Sender}};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct Engine {
    /// The instrument the engine processes
    instrument: Instrument,
    /// Manages orders
    book: OrderBook,
    /// Evaluates incoming orders
    risk: RiskAnalyser,
    /// Receives orders from the gateway
    order_receiver: Receiver<Request>,
    /// Used to publish an execution for a client
    executions: Sender<OutboundExecution>,
    /// Next execution sequence for each client
    sequences: ExecutionSequence,
    metrics: OrderbookMetrics,
}

impl Engine {
    pub fn new(
        instrument: Instrument,
        order_receiver: Receiver<Request>,
        executions: Sender<OutboundExecution>,
        metrics: OrderbookMetrics,
    ) -> Self {
        Self {
            instrument: instrument.clone(),
            book: OrderBook::new(),
            risk: RiskAnalyser::new(),
            order_receiver,
            executions,
            sequences: ExecutionSequence::new(),
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

                    let published = match request {
                        Request::Place { instrument, price, order } => {
                            self.handle_place(instrument, price, order).await
                        }
                        Request::Cancel { client_id, order_id, price, side } => {
                            self.handle_cancel(client_id, order_id, price, side).await
                        }
                    };

                    if let Err(error) = published {
                        error!(instrument = %self.instrument, %error, "Failed to publish execution");
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    /// Processes a new order / trade request
    async fn handle_place(
        &mut self,
        instrument: Instrument,
        price: Price,
        order: LimitOrder,
    ) -> Result<()> {
        // Determine if this order passes arbitrary safety checks or if it must be rejected
        match self.risk.evaluate(&order, &price) {
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
                self.publish(client_id, Response::OrderRejected(rejection))
                    .await?;

                return Ok(());
            }
        }

        // Local var used for logging quantity filled
        let requested_size = order.size;

        // Perform the trade/insert into book
        let result = self.book.trade(price, order.clone());

        let mut filled = requested_size;
        filled -= result.remaining;

        let trade_count = result.matches.len() as u64;

        if let Err(error) = self
            .metrics
            .record_orderbook(&self.book, trade_count, filled)
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
            filled_size = %filled,
            %remaining,
            trade_count = result.matches.len(),
            side = %order.side,
            status = %result.status(),
            client = %order.client_id,
            order = %order.order_id,
            "Order processed"
        );

        // Publish results to connected clients about their trades
        for order_match in result.matches {
            self.publish(
                order_match.maker.client_id,
                Response::Trade(Trade::new(order_match.price, &order_match.maker)),
            )
            .await?;
            self.publish(
                order_match.taker.client_id,
                Response::Trade(Trade::new(order_match.price, &order_match.taker)),
            )
            .await?;
        }

        // If the incoming order is partially filled we store the remainder in the book
        // Therefore, inform the client that we have stored the remainder for future trades
        if Quantity::ZERO < remaining {
            self.publish(
                order.client_id,
                Response::OrderAccepted(OrderAccepted {
                    exec_seq: 0,
                    order_id: order.order_id,
                }),
            )
            .await?;
        }

        Ok(())
    }

    /// Removes an existing order from the orderbook
    async fn handle_cancel(
        &mut self,
        client_id: Uuid,
        order_id: Uuid,
        price: Price,
        side: OrderType,
    ) -> Result<()> {
        let cancelled = self.book.cancel(client_id, order_id, price, side);

        if cancelled {
            info!(
                instrument = %self.instrument,
                %client_id,
                order = %order_id,
                %price,
                %side,
                "Order cancelled"
            );
            self.publish(
                client_id,
                Response::Cancelled(Cancelled {
                    exec_seq: 0,
                    order_id,
                }),
            )
            .await?;
        } else {
            warn!(
                instrument = %self.instrument,
                %client_id,
                order = %order_id,
                %price,
                %side,
                "Cancel rejected"
            );
            self.publish(
                client_id,
                Response::CancelRejected(CancelRejection {
                    exec_seq: 0,
                    order_id,
                    reason: CancelRejectionReason::OrderNotFound,
                }),
            )
            .await?;
        }

        Ok(())
    }

    async fn publish(&mut self, client_id: Uuid, mut response: Response) -> Result<()> {
        response.set_exec_seq(self.sequences.next(client_id));
        let payload = serde_json::to_string(&response).map_err(|error| eyre!(error))?;

        self.executions
            .send(OutboundExecution { client_id, payload })
            .await
            .map_err(|error| eyre!(error))
    }
}
