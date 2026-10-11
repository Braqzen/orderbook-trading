//! Process environment for the market data provider gateway.

use eyre::Result;

pub struct Environment {
    /// Bind to this socket to accept client WebSocket connections
    pub ws: String,
    /// Address of the Valkey service the gateway connects to
    pub valkey_url: String,
}

impl Environment {
    pub fn load() -> Result<Self> {
        Ok(Self {
            ws: std::env::var("WS")?,
            valkey_url: std::env::var("VALKEY_URL")?,
        })
    }
}
