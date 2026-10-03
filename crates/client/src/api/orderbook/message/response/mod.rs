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

use cancel::CancelRejection;
use serde::Deserialize;

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
