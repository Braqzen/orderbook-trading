//! Tracks instrument subscriptions per user
//!
//! # SAFETY
//!
//! Clients may only subscribe to instruments listed in the gateway config. A second subscribe to the
//! same instrument is rejected. There is no global cap on client count; many connections subscribing
//! may still OOM the gateway.

use crate::{
    api::{
        price::{PriceChannels, PriceUpdate},
        websocket::request::Operation,
    },
    whitelist::InstrumentWhitelist,
};
use tokio_stream::{
    StreamExt, StreamMap,
    wrappers::{BroadcastStream, errors::BroadcastStreamRecvError},
};

pub struct Subscriptions {
    /// Instruments a client may subscribe to
    whitelist: InstrumentWhitelist,
    /// Instrument key to its channel price receiver
    streams: StreamMap<String, BroadcastStream<PriceUpdate>>,
}

impl Subscriptions {
    pub fn new(whitelist: InstrumentWhitelist) -> Self {
        Self {
            whitelist,
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
                    if !self.whitelist.contains(instrument) {
                        rejected.push(instrument.clone());
                        continue;
                    }

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
