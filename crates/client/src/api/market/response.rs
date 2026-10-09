//! Expected responses from the market data provider
//!
//! The provider may send a price update for a subscribed instrument, information about the state of a subscription request,
//! or a fallback rejection if it could not handle the request for an unexpected reason.
//!
//! Currently, it does not include information about the fallback so client cannot attempt to recover.

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum Response {
    Price(PriceUpdate),
    Subscription(SubscriptionResponse),
    Rejected,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum SubscriptionResponse {
    /// All requested instruments have been subscribed
    Subscribed(Vec<String>),
    /// Only some of the requested instruments have been subscribed
    /// Contains which instruments have been rejected
    PartiallySubscribed {
        subscribed: Vec<String>,
        rejected: Vec<String>,
    },
    /// All requested instruments have been unsubscribed
    Unsubscribed(Vec<String>),
    /// Only some of the requested instruments have been unsubscribed
    /// Contains which instruments have been rejected
    PartiallyUnsubscribed {
        unsubscribed: Vec<String>,
        rejected: Vec<String>,
    },
    /// Subscription request has been completely rejected for all provided instruments
    Rejected(Vec<String>),
}

#[derive(Deserialize)]
pub struct PriceUpdate {
    pub instrument: String,
    pub value: f64,
}
