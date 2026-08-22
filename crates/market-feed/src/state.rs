use crate::proto::PriceUpdate;
use std::collections::HashMap;
use tokio::sync::{RwLock, broadcast::Sender};

pub struct State {
    pub prices: RwLock<HashMap<String, f64>>,
    pub price_sender_channel: Sender<PriceUpdate>,
}

impl State {
    pub fn new(price_sender_channel: Sender<PriceUpdate>) -> Self {
        Self {
            prices: RwLock::new(HashMap::new()),
            price_sender_channel,
        }
    }
}
