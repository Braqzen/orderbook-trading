//! Types associated with sending a request to an orderbook

use crate::trade::{Instrument, Order, OrderType, Price, Quantity};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct LoginRequest {
    client_id: Uuid,
}

impl LoginRequest {
    pub fn new(client_id: Uuid) -> Self {
        Self { client_id }
    }
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Request {
    Place {
        order_id: Uuid,
        instrument: Instrument,
        price: Price,
        size: Quantity,
        side: OrderType,
    },
    Cancel {
        order_id: Uuid,
        price: Price,
        side: OrderType,
    },
}

impl Request {
    pub fn place(order: Order) -> Self {
        Self::Place {
            order_id: order.order_id,
            instrument: order.instrument,
            price: order.price,
            size: order.size,
            side: order.side,
        }
    }

    pub fn cancel(order_id: Uuid, price: Price, side: OrderType) -> Self {
        Self::Cancel {
            order_id,
            price,
            side,
        }
    }
}

#[derive(Clone)]
pub struct RequestMetadata {
    pub instrument: Instrument,
    pub message: Request,
}
