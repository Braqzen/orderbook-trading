//! Wire types representing responses for actions the service has performed on behalf of the client

use crate::trade::{Instrument, LimitOrder, OrderType, Price, Quantity, RejectionReason, Trade};
use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Response {
    /// A [`Trade`] has occured
    Trade(Trade),
    /// An order has been added to the orderbook
    OrderAccepted(OrderAccepted),
    /// An order has been rejected and not added to the orderbook
    OrderRejected(OrderRejection),
    /// A cancel request has been accepted and an order has been removed from the book
    Cancelled(Cancelled),
    /// A cancel request has been rejected and an order has not been removed
    CancelRejected(CancelRejection),
    /// Client has successfully logged in
    LoginAccepted,
    /// A client login attempt has been rejected
    LoginRejected(LoginRejection),
}

impl Response {
    pub fn login_rejected(reason: LoginRejectionReason) -> Self {
        Self::LoginRejected(LoginRejection { reason })
    }
}

#[derive(Serialize)]
pub struct LoginRejection {
    pub reason: LoginRejectionReason,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginRejectionReason {
    ClientAlreadyConnected,
    NotLoggedIn,
}

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

#[derive(Serialize)]
pub struct Cancelled {
    pub order_id: Uuid,
}

#[derive(Serialize)]
pub struct CancelRejection {
    pub order_id: Uuid,
    pub reason: CancelRejectionReason,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelRejectionReason {
    OrderNotFound,
}
