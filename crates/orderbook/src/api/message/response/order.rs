use crate::trade::{Instrument, LimitOrder, OrderType, Price, Quantity, RejectionReason};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct OrderAccepted {
    pub order_id: Uuid,
}

#[derive(Serialize)]
pub struct OrderRejection {
    pub order_id: Uuid,
    pub instrument: Instrument,
    pub price: Price,
    pub size: Quantity,
    pub side: OrderType,
    pub reason: RejectionReason,
}

impl OrderRejection {
    pub fn new(
        instrument: Instrument,
        price: Price,
        order: LimitOrder,
        reason: RejectionReason,
    ) -> Self {
        Self {
            order_id: order.order_id,
            instrument,
            price,
            size: order.size,
            side: order.side,
            reason,
        }
    }
}
