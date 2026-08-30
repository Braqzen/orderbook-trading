use crate::trade::Quantity;

#[derive(Debug, Clone, Copy)]
pub struct TradeLimit {
    pub minimum_size: Quantity,
    pub maximum_size: Quantity,
}
