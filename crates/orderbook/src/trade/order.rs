use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display, Formatter},
    num::NonZeroU64,
};
use uuid::Uuid;

#[derive(Clone)]
pub struct LimitOrder {
    pub size: u64,
    pub side: OrderType,
    pub client_id: Uuid,
    pub order_id: Uuid,
}

impl LimitOrder {
    pub fn new(size: NonZeroU64, side: OrderType, client_id: Uuid, order_id: Uuid) -> Self {
        Self {
            size: size.get(),
            side,
            client_id,
            order_id,
        }
    }

    pub fn filled(&self) -> bool {
        self.size == 0
    }
}

#[derive(Clone, Copy, Deserialize, Eq, Hash, PartialEq, Serialize)]
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
