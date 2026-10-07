//! Messages sent to the orderbook service.
//!
//! [`LoginRequest`] logs the client in, and [`Request`] places or cancels an order.

mod login;
mod metadata;

pub use login::LoginRequest;
pub use metadata::RequestMetadata;

use crate::trade::{Instrument, Order, OrderType, Price, Quantity};
use serde::Serialize;
use uuid::Uuid;

/// Message serialized and sent to the orderbook service.
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
