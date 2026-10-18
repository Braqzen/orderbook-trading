//! Wire types representing responses for actions the service has performed on behalf of the client

mod cancel;
mod order;
mod trade;

pub use cancel::{CancelRejection, CancelRejectionReason, Cancelled};
pub use order::{OrderAccepted, OrderRejection};
use serde::Serialize;
pub use trade::Trade;

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
}

impl Response {
    pub fn set_exec_seq(&mut self, exec_seq: u64) {
        match self {
            Self::Trade(trade) => trade.exec_seq = exec_seq,
            Self::OrderAccepted(accepted) => accepted.exec_seq = exec_seq,
            Self::OrderRejected(rejection) => rejection.exec_seq = exec_seq,
            Self::Cancelled(cancelled) => cancelled.exec_seq = exec_seq,
            Self::CancelRejected(rejection) => rejection.exec_seq = exec_seq,
        }
    }
}
