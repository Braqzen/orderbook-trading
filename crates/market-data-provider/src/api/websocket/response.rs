//! Responses sent to the client
//!
//! The provider may send a price update for a subscribed instrument, information about the state of a subscription request,
//! or a fallback rejection if it could not handle the request for an unexpected reason.
//!
//! Currently, it does not include information about the fallback so client cannot attempt to recover.

use crate::api::websocket::request::Operation;
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "data")]
pub enum Response {
    Price(PriceUpdate),
    Subscription(SubscriptionResponse),
    Rejected,
}

#[derive(Serialize)]
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

impl Response {
    pub fn price(price: crate::proto::PriceUpdate) -> Self {
        Self::Price(PriceUpdate {
            instrument: price.instrument,
            value: price.value,
        })
    }

    pub fn subscription(
        operation: Operation,
        accepted: Vec<String>,
        rejected: Vec<String>,
    ) -> Self {
        let response = match (operation, accepted.is_empty(), rejected.is_empty()) {
            (Operation::Subscribe, false, true) => SubscriptionResponse::Subscribed(accepted),
            (Operation::Subscribe, false, false) => SubscriptionResponse::PartiallySubscribed {
                subscribed: accepted,
                rejected,
            },
            (Operation::Unsubscribe, false, true) => SubscriptionResponse::Unsubscribed(accepted),
            (Operation::Unsubscribe, false, false) => SubscriptionResponse::PartiallyUnsubscribed {
                unsubscribed: accepted,
                rejected,
            },
            (_, true, false) => SubscriptionResponse::Rejected(rejected),
            (_, true, true) => return Self::Rejected,
        };

        Self::Subscription(response)
    }
}

#[derive(Serialize)]
pub struct PriceUpdate {
    instrument: String,
    value: f64,
}
