use serde::Deserialize;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy)]
pub struct Order {
    pub size: u64,
    pub side: OrderType,
}

impl Order {
    pub fn new(size: u64, side: OrderType) -> Self {
        Self { size, side }
    }
}

#[derive(Clone, Copy, Deserialize)]
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
