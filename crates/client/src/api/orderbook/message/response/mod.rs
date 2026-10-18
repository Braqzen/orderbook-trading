//! Messages received from the orderbook service.
//!
//! [`LoginResponse`] is the login result.
//! [`Response`] is related to trading actions.

mod cancel;
mod login;
mod order;
mod trade;

pub use cancel::Cancelled;
pub use login::LoginResponse;
pub use order::{OrderAccepted, OrderRejection};
pub use trade::Trade;

use crate::trade::Instrument;
use cancel::CancelRejection;
use serde::Deserialize;

pub struct ExecutionReport {
    pub instrument: Instrument,
    pub response: Response,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Response {
    /// An order has been traded.
    Trade(Trade),
    /// An order was added to the book.
    OrderAccepted(OrderAccepted),
    /// A request to place an order was rejected.
    OrderRejected(OrderRejection),
    /// An order was removed from the book.
    Cancelled(Cancelled),
    /// A request to remove an order from the book has been rejected.
    CancelRejected(CancelRejection),
}

impl Response {
    pub fn exec_seq(&self) -> u64 {
        match self {
            Self::Trade(trade) => trade.exec_seq,
            Self::OrderAccepted(accepted) => accepted.exec_seq,
            Self::OrderRejected(rejection) => rejection.exec_seq,
            Self::Cancelled(cancelled) => cancelled.exec_seq,
            Self::CancelRejected(rejection) => rejection.exec_seq,
        }
    }
}
