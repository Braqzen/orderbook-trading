use crate::{instrument::Instrument, simulation::PriceConfig};
use eyre::{Result, ensure, eyre};
use serde::Deserialize;
use std::path::Path;

pub struct Config {
    pub feeds: Vec<InstrumentFeed>,
}

impl Config {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .map_err(|error| eyre!("Failed to read config at {}: {error}", path.display()))?;
        let feeds: Vec<InstrumentFeed> = serde_json::from_str(&contents)
            .map_err(|error| eyre!("Failed to parse config at {}: {error}", path.display()))?;

        ensure!(!feeds.is_empty(), "Config must contain at least one feed");

        Ok(Self { feeds })
    }
}

/// Each entry in the config consists of an identifier, a start price and price constraints
#[derive(Debug, Deserialize)]
pub struct InstrumentFeed {
    symbol: String,
    start_price: f64,
    upper_limit: f64,
    lower_limit: f64,
}

impl InstrumentFeed {
    pub fn instrument(&self) -> Result<Instrument> {
        Instrument::try_from(self.symbol.as_str())
    }

    pub fn price_config(&self) -> Result<PriceConfig> {
        PriceConfig::new(self.start_price, self.upper_limit, self.lower_limit)
    }
}
