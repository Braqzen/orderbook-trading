//! Instrument-specific channels between Valkey and subscribed WebSocket connections.
//!
//! When a client subscribes to an instrument a single sender channel is created for that instrument
//! and a receiver handle is given to that client.
//!
//! New subscriptions to the same instrument do not create new sender channels, there is 1 sender per instrument,
//! but a new receiver channel is created for the client.
//!
//! This allows clients to create channels for instruments where a price is sent only through the channel for that instrument,
//! thus allowing clients to subscribe/unsubscribe to specific instrument updates by keeping/dropping the receiver for that instrument.

use std::{collections::HashMap, sync::Arc};
use tokio::sync::{
    RwLock,
    broadcast::{self, Receiver, Sender},
};

#[derive(Clone)]
pub struct PriceUpdate {
    pub instrument: String,
    pub value: f64,
}

#[derive(Clone)]
pub struct PriceChannels {
    /// Price channel per instrument i.e. "BTC-USD: Sender Channel"
    channels: Arc<RwLock<HashMap<String, Sender<PriceUpdate>>>>,
}

impl PriceChannels {
    pub fn new() -> Self {
        Self {
            channels: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Creates a new channel to send instrument updates through handing out a receiver for that specific instrument
    ///
    /// # SAFETY
    ///
    /// Unbounded creation. Project does 0 checks to limit number of instruments which may lead to OOM in real systems.
    /// While the instrument whitelist is a limit, with many clients that may add up to OOM (not in this demo though)
    pub async fn subscribe(&self, instrument: &str) -> Receiver<PriceUpdate> {
        let mut channels = self.channels.write().await;
        channels
            .entry(instrument.to_owned())
            .or_insert_with(|| broadcast::channel(128).0)
            .subscribe()
    }

    /// Publish an update through a specific instrument channel returning the number of recipients who received the update
    pub async fn publish(&self, price: PriceUpdate) -> usize {
        let channels = self.channels.read().await;
        let Some(channel) = channels.get(&price.instrument) else {
            return 0;
        };

        match channel.send(price) {
            Ok(delivered) => delivered,
            Err(_) => 0,
        }
    }
}
