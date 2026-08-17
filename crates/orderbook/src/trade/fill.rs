use crate::trade::{Order, Price};

pub struct Fill {
    pub price: Price,
    pub size: u64,
}

impl Fill {
    pub fn new(price: Price, size: u64) -> Self {
        Self { price, size }
    }
}

pub enum ExecutionResult {
    Filled { fills: Vec<Fill> },
    PartiallyFilled { fills: Vec<Fill>, remainder: Order },
    Unfilled { order: Order },
}
