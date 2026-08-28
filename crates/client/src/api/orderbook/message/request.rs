use crate::trade::{Instrument, Order, OrderType, Price, Quantity};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize, Clone)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Request {
    Place {
        client_id: Uuid,
        order_id: Uuid,
        instrument: Instrument,
        price: Price,
        size: Quantity,
        side: OrderType,
    },
    Cancel {
        client_id: Uuid,
        order_id: Uuid,
        price: Price,
        side: OrderType,
    },
}

impl Request {
    pub fn place(order: Order) -> Self {
        Self::Place {
            client_id: order.client_id,
            order_id: order.order_id,
            instrument: order.instrument,
            price: order.price,
            size: order.size,
            side: order.side,
        }
    }

    pub fn cancel(client_id: Uuid, order_id: Uuid, price: Price, side: OrderType) -> Self {
        Self::Cancel {
            client_id,
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
