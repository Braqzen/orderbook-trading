//! Process environment for the market data provider.

use eyre::Result;

pub struct Environment {
    /// Bind to this socket to listen for generator instrument/price updates via gRPC
    pub socket: String,
    /// Address of the Valkey service the provider connects to
    pub valkey_url: String,
    /// Seconds each stored price is kept before Valkey expires it
    pub ttl_seconds: String,
}

impl Environment {
    pub fn load() -> Result<Self> {
        Ok(Self {
            socket: std::env::var("SOCKET")?,
            valkey_url: std::env::var("VALKEY_URL")?,
            ttl_seconds: std::env::var("PRICE_TTL_SECONDS")?,
        })
    }
}
