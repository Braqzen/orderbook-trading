//! Business logic handling order submissions from the orderbook gateway.

use crate::{
    metrics::OrderbookMetrics,
    proto::{self, SubmitRequest, orderbook_server::Orderbook},
    trade::{Instrument, LimitOrder, OrderType, Price, Quantity, Request},
};
use std::num::NonZeroU64;
use tokio::sync::mpsc::{Sender, error::TrySendError};
use tonic::{Request as GrpcRequest, Response, Status};
use uuid::Uuid;

pub struct OrderbookService {
    instrument: Instrument,
    orders: Sender<Request>,
    metrics: OrderbookMetrics,
}

impl OrderbookService {
    pub fn new(instrument: Instrument, orders: Sender<Request>, metrics: OrderbookMetrics) -> Self {
        Self {
            instrument,
            orders,
            metrics,
        }
    }

    fn request(&self, submit: SubmitRequest) -> Result<Request, Status> {
        let client_id = Uuid::parse_str(&submit.client_id)
            .map_err(|_| Status::invalid_argument("invalid client id"))?;

        match submit.action {
            Some(proto::submit_request::Action::Place(place)) => {
                self.place(client_id, place)
            }
            Some(proto::submit_request::Action::Cancel(cancel)) => {
                self.cancel(client_id, cancel)
            }
            None => Err(Status::invalid_argument("missing order action")),
        }
    }

    fn place(&self, client_id: Uuid, place: proto::Place) -> Result<Request, Status> {
        let instrument = Instrument::try_from(place.instrument.as_str())
            .map_err(|_| Status::invalid_argument("invalid instrument"))?;

        if instrument != self.instrument {
            return Err(Status::invalid_argument("invalid instrument"));
        }

        let price = self.nonzero(place.price, "invalid price")?;
        let size = self.nonzero(place.size, "invalid order size")?;
        let order_size = Quantity::from(size.get());

        if !order_size.valid_size() {
            return Err(Status::invalid_argument("invalid order size"));
        }

        let order_id = Uuid::parse_str(&place.order_id)
            .map_err(|_| Status::invalid_argument("invalid order id"))?;

        Ok(Request::Place {
            instrument,
            price: Price::from(price),
            order: LimitOrder::new(order_size, self.side(place.side)?, client_id, order_id),
        })
    }

    fn cancel(&self, client_id: Uuid, cancel: proto::Cancel) -> Result<Request, Status> {
        let order_id = Uuid::parse_str(&cancel.order_id)
            .map_err(|_| Status::invalid_argument("invalid order id"))?;
        let price = self.nonzero(cancel.price, "invalid price")?;

        Ok(Request::Cancel {
            client_id,
            order_id,
            price: Price::from(price),
            side: self.side(cancel.side)?,
        })
    }

    fn nonzero(&self, value: u64, message: &str) -> Result<NonZeroU64, Status> {
        NonZeroU64::new(value).ok_or_else(|| Status::invalid_argument(message))
    }

    fn side(&self, side: i32) -> Result<OrderType, Status> {
        match proto::Side::try_from(side) {
            Ok(proto::Side::Buy) => Ok(OrderType::Buy),
            Ok(proto::Side::Sell) => Ok(OrderType::Sell),
            _ => Err(Status::invalid_argument("invalid side")),
        }
    }
}

#[tonic::async_trait]
impl Orderbook for OrderbookService {
    async fn submit(
        &self,
        request: GrpcRequest<SubmitRequest>,
    ) -> Result<Response<()>, Status> {
        let request = self.request(request.into_inner())?;

        match self.orders.try_send(request) {
            Ok(()) => {
                self.metrics.global_order_enqueued();
                Ok(Response::new(()))
            }
            Err(TrySendError::Full(_)) => Err(Status::resource_exhausted("order queue full")),
            Err(TrySendError::Closed(_)) => Err(Status::unavailable("order channel closed")),
        }
    }
}
