use crate::trade::{ExecutionResult, OrderBook, Request, Response, RiskAnalyser};
use eyre::Result;
use tokio::{select, sync::mpsc::Receiver};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub struct Engine {
    book: OrderBook,
    risk: RiskAnalyser,
}

impl Engine {
    pub fn new(instrument: String) -> Self {
        Self {
            book: OrderBook::new(),
            risk: RiskAnalyser::new(instrument),
        }
    }

    pub async fn run(
        &mut self,
        mut receiver: Receiver<Request>,
        token: CancellationToken,
    ) -> Result<()> {
        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                request = receiver.recv() => {
                    let Some(request) = request else {
                        error!("Engine to orderbook api channel closed");
                        break;
                    };

                    match self
                        .risk
                        .evaluate(&request.instrument, &request.order, &request.price)
                    {
                        Ok(()) => {}
                        Err(reason) => {
                            warn!(
                                instrument = %request.instrument,
                                order = %request.order.order_id,
                                "Order rejected"
                            );
                            if request
                                .response
                                .send(Response::Rejected {
                                    order_id: request.order.order_id,
                                    reason,
                                })
                                .is_err()
                            {
                                warn!("Client disconnected before receiving rejection");
                            }
                            continue;
                        }
                    }

                    let Request {
                        price,
                        order,
                        response: response_sender,
                        ..
                    } = request;

                    let requested_size = order.size;
                    let order_id = order.order_id;
                    let trades = self.book.trade(price, order.clone());

                    let (response, status, fill_count, remaining_size) = match trades {
                        ExecutionResult::Filled { fills } => (
                            Response::Filled {
                                order_id,
                                filled_size: requested_size,
                            },
                            "filled",
                            fills.len(),
                            0,
                        ),
                        ExecutionResult::PartiallyFilled { fills, remainder } => {
                            let remaining_size = remainder.size;
                            let filled_size = requested_size - remaining_size;

                            (
                                Response::PartiallyFilled {
                                    order_id,
                                    filled_size,
                                    remaining_size,
                                },
                                "partial",
                                fills.len(),
                                remaining_size,
                            )
                        }
                        ExecutionResult::Unfilled { .. } => (
                            Response::Unfilled { order_id },
                            "unfilled",
                            0,
                            requested_size,
                        ),
                    };
                    let filled_size = requested_size - remaining_size;

                    info!(
                        limit_price = %price,
                        requested_size,
                        filled_size,
                        remaining_size,
                        fill_count,
                        side = %order.side,
                        status,
                        client=%order.client_id,
                        order=%order_id,
                        "Order processed"
                    );

                    if response_sender.send(response).is_err() {
                        warn!("Client disconnected before receiving order response");
                    }
                }
            }
        }

        Ok(())
    }
}
