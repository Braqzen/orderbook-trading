//! Process environment for the trading client.

use eyre::Result;

pub struct Environment {
    /// Path to the client instrument, inventory, and limit config
    pub config_path: String,
    /// WebSocket address of the market data gateway
    pub market_data_gateway_url: String,
    /// Address of the Valkey service the client stores its execution sequence in
    pub valkey_url: String,
}

impl Environment {
    pub fn load() -> Result<Self> {
        Ok(Self {
            config_path: std::env::var("CONFIG_PATH")?,
            market_data_gateway_url: std::env::var("MARKET_DATA_GATEWAY_URL")?,
            valkey_url: std::env::var("VALKEY_URL")?,
        })
    }
}
