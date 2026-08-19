use crate::trade::Instrument;
use serde::Serialize;
use std::fmt::{self, Display, Formatter};
use uuid::Uuid;

#[derive(Serialize, Clone)]
pub struct Order {
    pub client_id: Uuid,
    pub order_id: Uuid,
    pub instrument: Instrument,
    pub price: f64,
    pub size: u64,
    pub side: OrderType,
}

impl Order {
    pub fn new(
        instrument: Instrument,
        price: f64,
        size: u64,
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

#[derive(Serialize, Clone, Copy)]
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
