//! Reports whether a place request was written to the orderbook gateway websocket.
//!
//! The trader reserves inventory before routing; it must not treat the order as open until the
//! orderbook task confirms the websocket send. Accepted and rejected replies from the orderbook
//! service arrive later/async; this message is only for immediate confirm or rollback when routing or
//! sending to the orderbook fails.

use crate::trade::Order;

#[derive(Clone)]
pub struct SubmitOutcome {
    pub order: Order,
    pub submitted: bool,
}
