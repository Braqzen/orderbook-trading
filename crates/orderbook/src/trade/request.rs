use crate::trade::{Instrument, LimitOrder, OrderType, Price};
use uuid::Uuid;

pub enum Request {
    Place {
        instrument: Instrument,
        price: Price,
        order: LimitOrder,
    },
    Cancel {
        client_id: Uuid,
        order_id: Uuid,
        price: Price,
        side: OrderType,
    },
}

impl Request {
    pub fn client_id(&self) -> Uuid {
        match self {
            Self::Place { order, .. } => order.client_id,
            Self::Cancel { client_id, .. } => *client_id,
        }
    }
}
