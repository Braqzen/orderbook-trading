use crate::trade::Instrument;
use eyre::{Result, ensure, eyre};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fmt::{self, Display, Formatter},
    path::Path,
};

pub struct Config {
    pub instruments: HashMap<Instrument, WsUrl>,
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

        let instruments = parse_instruments(raw.instrument)?;

        Ok(Self {
            instruments,
            inventory: raw.inventory,
        })
    }
}

fn parse_instruments(raw: HashMap<String, String>) -> Result<HashMap<Instrument, WsUrl>> {
    let mut instruments = HashMap::with_capacity(raw.len());

    for (instrument, url) in raw {
        let instrument = Instrument::try_from(instrument.as_str())
            .map_err(|error| eyre!("Invalid instrument in config: {error}"))?;
        let url = WsUrl::try_from(url.as_str())
            .map_err(|error| eyre!("Invalid orderbook URL for {instrument}: {error}"))?;

        ensure!(
            !instruments.contains_key(&instrument),
            "Duplicate instrument in config: {instrument}"
        );

        instruments.insert(instrument, url);
    }

    Ok(instruments)
}

#[derive(Debug, Clone)]
pub struct WsUrl(String);

impl WsUrl {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for WsUrl {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl TryFrom<&str> for WsUrl {
    type Error = eyre::Report;

    fn try_from(value: &str) -> Result<Self> {
        ensure!(
            value.starts_with("ws://") || value.starts_with("wss://"),
            "Invalid websocket URL: {value}"
        );

        Ok(Self(value.to_owned()))
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InventoryEntry {
    pub symbol: String,
    pub upper_limit: f64,
    pub lower_limit: f64,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    instrument: HashMap<String, String>,
    inventory: Vec<InventoryEntry>,
}
