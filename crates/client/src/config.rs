use eyre::{Result, ensure, eyre};
use serde::Deserialize;
use std::path::Path;

pub struct Config {
    pub instruments: Vec<String>,
    pub inventory: Vec<InventoryEntry>,
}

impl Config {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .map_err(|error| eyre!("Failed to read config at {}: {error}", path.display()))?;
        let raw: RawConfig = serde_json::from_str(&contents)
            .map_err(|error| eyre!("Failed to parse config at {}: {error}", path.display()))?;

        ensure!(
            !raw.instrument.is_empty(),
            "Config must contain at least one instrument"
        );
        ensure!(
            !raw.inventory.is_empty(),
            "Config must contain at least one inventory entry"
        );

        Ok(Self {
            instruments: raw.instrument,
            inventory: raw.inventory,
        })
    }
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    instrument: Vec<String>,
    inventory: Vec<InventoryEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InventoryEntry {
    pub symbol: String,
    pub upper_limit: f64,
    pub lower_limit: f64,
}
