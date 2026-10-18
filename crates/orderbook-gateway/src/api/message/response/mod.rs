//! Wire types representing responses for actions the service has performed on behalf of the client

mod cancel;
mod login;
mod order;
mod trade;

pub use cancel::{CancelRejection, Cancelled};
pub use login::{LoginRejection, LoginRejectionReason};
pub use order::{OrderAccepted, OrderRejection};
use serde::{Deserialize, Serialize};
pub use trade::Trade;

#[derive(Deserialize, Serialize)]
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

    pub fn exec_seq(&self) -> Option<u64> {
        match self {
            Self::Trade(trade) => Some(trade.exec_seq),
            Self::OrderAccepted(accepted) => Some(accepted.exec_seq),
            Self::OrderRejected(rejection) => Some(rejection.exec_seq),
            Self::Cancelled(cancelled) => Some(cancelled.exec_seq),
            Self::CancelRejected(rejection) => Some(rejection.exec_seq),
            Self::LoginAccepted | Self::LoginRejected(_) => None,
        }
    }
}
