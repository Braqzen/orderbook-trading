//! Process environment for the orderbook gateway.

use eyre::Result;

pub struct Environment {
    /// Bind to this socket to accept client WebSocket connections
    pub ws: String,
    /// The only instrument this gateway accepts
    pub instrument: String,
    /// gRPC address of the orderbook for this instrument
    pub orderbook_url: String,
    /// Redpanda brokers the gateway reads executions from
    pub redpanda_brokers: String,
}

impl Environment {
    pub fn load() -> Result<Self> {
        Ok(Self {
            ws: std::env::var("WS")?,
            instrument: std::env::var("INSTRUMENT")?,
            orderbook_url: std::env::var("ORDERBOOK_URL")?,
            redpanda_brokers: std::env::var("REDPANDA_BROKERS")?,
        })
    }
}
