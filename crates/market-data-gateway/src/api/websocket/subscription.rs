//! Tracks instrument subscriptions per user
//!
//! # SAFETY
//!
//! There are no checks to validate what is or is not an instrument, client may add garbage atm
//! The list is also unbounded and may OOM the gateway

use crate::api::price::{PriceChannels, PriceUpdate};
use crate::api::websocket::request::Operation;
use tokio_stream::{
    StreamExt, StreamMap,
    wrappers::{BroadcastStream, errors::BroadcastStreamRecvError},
};

pub struct Subscriptions {
    /// Instrument key to its channel price receiver
    streams: StreamMap<String, BroadcastStream<PriceUpdate>>,
}

impl Subscriptions {
    pub fn new() -> Self {
        Self {
            streams: StreamMap::new(),
        }
    }

    pub async fn update(
        &mut self,
        prices: &PriceChannels,
        operation: Operation,
        instruments: &[String],
    ) -> (Vec<String>, Vec<String>) {
        let mut accepted = Vec::new();
        let mut rejected = Vec::new();

        match operation {
            Operation::Subscribe => {
                for instrument in instruments {
                    if self.streams.contains_key(instrument) {
                        rejected.push(instrument.clone());
                        continue;
                    }

                    let receiver = prices.subscribe(instrument).await;
                    self.streams
                        .insert(instrument.clone(), BroadcastStream::new(receiver));
                    accepted.push(instrument.clone());
                }
            }
            Operation::Unsubscribe => {
                for instrument in instruments {
                    if self.streams.remove(instrument).is_some() {
                        accepted.push(instrument.clone());
                    } else {
                        rejected.push(instrument.clone());
                    }
                }
            }
        }

        (accepted, rejected)
    }

    pub fn is_empty(&self) -> bool {
        self.streams.is_empty()
    }

    pub fn subscriptions(&self) -> impl Iterator<Item = &String> {
        self.streams.keys()
    }

    pub async fn next(
        &mut self,
    ) -> Option<(String, Result<PriceUpdate, BroadcastStreamRecvError>)> {
        self.streams.next().await
    }
}
