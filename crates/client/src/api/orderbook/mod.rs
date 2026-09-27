mod book;
mod connection;
mod message;

use crate::trade::Instrument;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{RwLock, mpsc::Sender};

pub use book::OrderBook;
pub use message::{
    Cancelled, LoginRequest, LoginResponse, OrderRejection, Request, RequestMetadata, Response,
    Trade,
};

// Only forward price updates to the engine if we are logged into that orderbook
pub type ActiveOrderbooks = Arc<RwLock<HashMap<Instrument, Sender<RequestMetadata>>>>;
