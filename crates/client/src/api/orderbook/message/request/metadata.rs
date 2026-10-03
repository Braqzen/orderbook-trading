//! Routes a [`Request`] to the orderbook connection for an instrument.
//!
//! This wrapper stays inside the client, and [`Request`] gets sent to the orderbook service.

use crate::{
    api::orderbook::message::request::Request,
    trade::{Instrument, Order},
};

#[derive(Clone)]
pub struct RequestMetadata {
    pub instrument: Instrument,
    pub message: Request,
}

impl RequestMetadata {
    pub fn place(order: &Order) -> Self {
        Self {
            instrument: order.instrument.clone(),
            message: Request::place(order.clone()),
        }
    }

    pub fn cancel(order: &Order) -> Self {
        Self {
            instrument: order.instrument.clone(),
            message: Request::cancel(order.order_id, order.price, order.side),
        }
    }
}
