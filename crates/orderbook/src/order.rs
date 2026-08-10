pub struct Order {
    size: u64,
    pub side: OrderType,
}

impl Order {
    pub fn new(size: u64, side: OrderType) -> Self {
        Self { size, side }
    }
}

pub enum OrderType {
    Buy,
    Sell,
}
