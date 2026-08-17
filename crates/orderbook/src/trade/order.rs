use serde::Deserialize;
use std::{
    fmt::{self, Display, Formatter},
    num::NonZeroU64,
};

#[derive(Clone, Copy)]
pub struct Order {
    pub size: u64,
    pub side: OrderType,
}

impl Order {
    pub fn new(size: NonZeroU64, side: OrderType) -> Self {
        Self {
            size: size.get(),
            side,
        }
    }

    pub fn filled(&self) -> bool {
        self.size == 0
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
