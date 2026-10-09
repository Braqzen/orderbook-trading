//! Tracks instrument subscriptions per user
//!
//! # SAFETY
//! There are no checks to validate what is or is not an instrument, client may add garbage atm
//! The list is also unbounded and may OOM the provider

use crate::api::websocket::request::Operation;
use std::collections::HashSet;

pub struct Subscriptions {
    subscribed: HashSet<String>,
}

impl Subscriptions {
    pub fn new() -> Self {
        Self {
            subscribed: HashSet::new(),
        }
    }

    pub fn update(
        &mut self,
        operation: Operation,
        instruments: &[String],
    ) -> (Vec<String>, Vec<String>) {
        let mut accepted = Vec::new();
        let mut rejected = Vec::new();

        match operation {
            Operation::Subscribe => {
                for instrument in instruments {
                    if self.subscribed.insert(instrument.clone()) {
                        accepted.push(instrument.clone());
                    } else {
                        rejected.push(instrument.clone());
                    }
                }
            }
            Operation::Unsubscribe => {
                for instrument in instruments {
                    if self.subscribed.remove(instrument) {
                        accepted.push(instrument.clone());
                    } else {
                        rejected.push(instrument.clone());
                    }
                }
            }
        }

        (accepted, rejected)
    }

    pub fn subscriptions(&self) -> impl Iterator<Item = &String> {
        self.subscribed.iter()
    }

    pub fn subscribed(&self, instrument: &str) -> bool {
        self.subscribed.contains(instrument)
    }
}
