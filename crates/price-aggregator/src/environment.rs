//! Process environment for the price aggregator.

use eyre::Result;

pub struct Environment {
    /// Address of the price generator the aggregator connects to for price updates
    pub price_generator_url: String,
    /// Address of the Valkey service the aggregator connects to
    pub valkey_url: String,
    /// Seconds each stored price is kept before Valkey expires it
    pub ttl_seconds: String,
}

impl Environment {
    pub fn load() -> Result<Self> {
        Ok(Self {
            price_generator_url: std::env::var("PRICE_GENERATOR_URL")?,
            valkey_url: std::env::var("VALKEY_URL")?,
            ttl_seconds: std::env::var("PRICE_TTL_SECONDS")?,
        })
    }
}
