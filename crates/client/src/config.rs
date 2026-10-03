use crate::{
    api::WsUrl,
    trade::{Asset, Instrument, ORDER_SIZE_ATOM_STEP, Quantity, TradeLimit},
};
use eyre::{Result, ensure, eyre};
use serde::Deserialize;
use std::{collections::HashMap, path::Path};

pub struct Config {
    pub instruments: HashMap<Instrument, WsUrl>,
    pub inventory: Vec<InventoryEntry>,
    pub trade_limits: HashMap<Asset, TradeLimit>,
}

impl Config {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)
            .map_err(|error| eyre!("Failed to read config at {}: {error}", path.display()))?;

        let config: RawConfig = serde_json::from_str(&contents)
            .map_err(|error| eyre!("Failed to parse config at {}: {error}", path.display()))?;

        ensure!(
            !config.instrument.is_empty(),
            "Config must contain at least one instrument"
        );
        ensure!(
            !config.inventory.is_empty(),
            "Config must contain at least one inventory entry"
        );
        ensure!(
            !config.trade_limit.is_empty(),
            "Config must contain at least one trade limit"
        );

        let instruments = parse_instruments(config.instrument)?;
        let trade_limits = parse_trade_limits(config.trade_limit)?;

        for instrument in instruments.keys() {
            ensure!(
                trade_limits.contains_key(instrument.base()),
                "Missing trade limit for {}",
                instrument.base().as_str()
            );
        }

        Ok(Self {
            instruments,
            inventory: config.inventory,
            trade_limits,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InventoryEntry {
    pub symbol: String,
    pub upper_limit: f64,
    pub lower_limit: f64,
}

fn parse_instruments(raw: HashMap<String, String>) -> Result<HashMap<Instrument, WsUrl>> {
    let mut instruments = HashMap::with_capacity(raw.len());

    for (instrument, url) in raw {
        let instrument = Instrument::try_from(instrument.as_str())
            .map_err(|error| eyre!("Invalid instrument in config: {error}"))?;
        let url = WsUrl::try_from(url.as_str())
            .map_err(|error| eyre!("Invalid orderbook URL for {instrument}: {error}"))?;

        instruments.insert(instrument, url);
    }

    Ok(instruments)
}

fn parse_trade_limits(raw: HashMap<String, RawTradeLimit>) -> Result<HashMap<Asset, TradeLimit>> {
    let mut trade_limits = HashMap::with_capacity(raw.len());

    for (symbol, limit) in raw {
        let minimum_size = Quantity::try_from(limit.minimum_size)
            .map_err(|error| eyre!("Invalid minimum trade size for {symbol}: {error}"))?;
        let maximum_size = Quantity::try_from(limit.maximum_size)
            .map_err(|error| eyre!("Invalid maximum trade size for {symbol}: {error}"))?;

        // TODO: fix this const being used outside file
        ensure!(
            minimum_size.atoms() % ORDER_SIZE_ATOM_STEP == 0
                && maximum_size.atoms() % ORDER_SIZE_ATOM_STEP == 0,
            "Trade limits for {symbol} must use at most six decimal places"
        );

        trade_limits.insert(
            Asset::new(symbol),
            TradeLimit {
                minimum_size,
                maximum_size,
            },
        );
    }

    Ok(trade_limits)
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    instrument: HashMap<String, String>,
    inventory: Vec<InventoryEntry>,
    trade_limit: HashMap<String, RawTradeLimit>,
}

#[derive(Debug, Deserialize)]
struct RawTradeLimit {
    minimum_size: f64,
    maximum_size: f64,
}
