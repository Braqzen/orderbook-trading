//! The trader's record of an order.
//!
//! The engine copies its fields into the request sent to the orderbook service.

use crate::trade::{Instrument, Price, Quantity};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};
use uuid::Uuid;

#[derive(Serialize, Clone)]
pub struct Order {
    /// ID representing the client that placed the order
    pub client_id: Uuid,
    /// ID representing a unique order
    pub order_id: Uuid,
    /// Instrument identifying the trading pair
    pub instrument: Instrument,
    /// The price at which to trade
    pub price: Price,
    /// The quantity desired to be traded
    pub size: Quantity,
    /// The type of order
    pub side: OrderType,
}

impl Order {
    pub fn new(
        instrument: Instrument,
        price: Price,
        size: Quantity,
        side: OrderType,
        client_id: Uuid,
        order_id: Uuid,
    ) -> Self {
        Self {
            instrument,
            price,
            size,
            side,
            client_id,
            order_id,
        }
    }
}

#[derive(Deserialize, Serialize, Clone, Copy)]
pub enum OrderType {
    Buy,
    Sell,
}

impl Display for OrderType {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Buy => formatter.write_str("Buy"),
            Self::Sell => formatter.write_str("Sell"),
        }
    }
}
