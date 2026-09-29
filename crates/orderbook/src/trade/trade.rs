use crate::trade::{LimitOrder, OrderType, Price, Quantity};
use std::fmt::{self, Display, Formatter};
use uuid::Uuid;

/// Internal type for the orderbook representing a unit of trade information
pub struct Fill {
    pub client_id: Uuid,
    pub order_id: Uuid,
    pub side: OrderType,
    pub fill_size: Quantity,
    pub remaining: Quantity,
}

impl Fill {
    pub fn new(order: &LimitOrder, fill_size: Quantity) -> Self {
        Self {
            client_id: order.client_id,
            order_id: order.order_id,
            side: order.side,
            fill_size,
            remaining: order.size,
        }
    }
}

/// Wrapper storing information about both sides of the trade
pub struct Match {
    pub price: Price,
    pub maker: Fill,
    pub taker: Fill,
}

impl Match {
    pub fn new(price: Price, maker: &LimitOrder, taker: &LimitOrder, fill_size: Quantity) -> Self {
        Self {
            price,
            maker: Fill::new(maker, fill_size),
            taker: Fill::new(taker, fill_size),
        }
    }
}

/// When a trade has occurred the status indicates the quantity of size that has been traded
pub enum TradeStatus {
    Unfilled,
    Partial,
    Filled,
}

impl Display for TradeStatus {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unfilled => formatter.write_str("unfilled"),
            Self::Partial => formatter.write_str("partial"),
            Self::Filled => formatter.write_str("filled"),
        }
    }
}

/// Single wrapper returning information about the outcome of a trade
pub struct TradeResult {
    pub matches: Vec<Match>,
    pub remaining: Quantity,
}

impl TradeResult {
    pub fn new(matches: Vec<Match>, remaining: Quantity) -> Self {
        Self { matches, remaining }
    }

    pub fn status(&self) -> TradeStatus {
        if self.matches.is_empty() {
            TradeStatus::Unfilled
        } else if Quantity::ZERO < self.remaining {
            TradeStatus::Partial
        } else {
            TradeStatus::Filled
        }
    }
}
