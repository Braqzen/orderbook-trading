use serde::Serialize;
use std::fmt::{self, Display, Formatter};

#[derive(Serialize, Clone, Copy)]
pub struct Order {
    pub price: f64,
    pub size: u64,
    pub side: OrderType,
}

impl Order {
    pub fn new(price: f64, size: u64, side: OrderType) -> Self {
        Self { price, size, side }
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
