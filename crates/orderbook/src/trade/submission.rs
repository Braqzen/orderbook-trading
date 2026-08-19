use crate::trade::{Instrument, Order, Price, risk::RejectionReason};
use serde::Serialize;
use tokio::sync::oneshot::Sender;
use uuid::Uuid;

pub struct Request {
    pub instrument: Instrument,
    pub price: Price,
    pub order: Order,
    pub response: Sender<Response>,
}

impl Request {
    pub fn new(
        instrument: Instrument,
        price: Price,
        order: Order,
        response: Sender<Response>,
    ) -> Self {
        Self {
            instrument,
            price,
            order,
            response,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Response {
    Rejected {
        order_id: Uuid,
        reason: RejectionReason,
    },
    Unfilled {
        order_id: Uuid,
    },
    PartiallyFilled {
        order_id: Uuid,
        filled_size: u64,
        remaining_size: u64,
    },
    Filled {
        order_id: Uuid,
        filled_size: u64,
    },
}
