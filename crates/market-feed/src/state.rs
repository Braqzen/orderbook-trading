use crate::proto::PriceUpdate;
use tokio::sync::{RwLock, broadcast::Sender};

pub struct State {
    pub current_price: RwLock<f64>,
    pub price_channel: Sender<PriceUpdate>,
}

impl State {
    pub fn new(current_price: RwLock<f64>, price_channel: Sender<PriceUpdate>) -> Self {
        Self {
            current_price,
            price_channel,
        }
    }
}
