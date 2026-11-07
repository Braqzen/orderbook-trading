//! Stores request routes for orderbook tasks with active websocket sessions.

use crate::{api::orderbook::RequestMetadata, trade::Instrument};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{RwLock, mpsc::Sender};

#[derive(Clone)]
pub struct OrderBookRegistry {
    /// Routes engine place and cancel requests to the logged-in orderbook task for each instrument
    routes: Arc<RwLock<HashMap<Instrument, Sender<RequestMetadata>>>>,
}

impl OrderBookRegistry {
    pub fn new() -> Self {
        Self {
            routes: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Associate the sender channel with an instrument
    pub async fn register(&self, instrument: Instrument, sender: Sender<RequestMetadata>) {
        self.routes.write().await.insert(instrument, sender);
    }

    /// Get the sender channel for an instrument
    pub async fn sender(&self, instrument: &Instrument) -> Option<Sender<RequestMetadata>> {
        self.routes.read().await.get(instrument).cloned()
    }

    /// Remove the entry for an instrument
    pub async fn remove(&self, instrument: &Instrument, sender: &Sender<RequestMetadata>) {
        let mut routes = self.routes.write().await;
        if routes
            .get(instrument)
            .is_some_and(|registered| registered.same_channel(sender))
        {
            routes.remove(instrument);
        }
    }

    /// Only continue a workflow if an instrument is logged in
    pub async fn connected(&self, instrument: &Instrument) -> bool {
        self.routes.read().await.contains_key(instrument)
    }
}
