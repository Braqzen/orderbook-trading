use crate::trade::{Asset, Instrument, ORDER_SIZE_ATOM_STEP, Quantity};
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
    pub trade_limits: HashMap<Asset, TradeLimit>,
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
        ensure!(
            !raw.trade_limit.is_empty(),
            "Config must contain at least one trade limit"
        );

        let instruments = parse_instruments(raw.instrument)?;
        let trade_limits = parse_trade_limits(raw.trade_limit)?;

        for instrument in instruments.keys() {
            ensure!(
                trade_limits.contains_key(instrument.base()),
                "Missing trade limit for {}",
                instrument.base().as_str()
            );
        }

        Ok(Self {
            instruments,
            inventory: raw.inventory,
            trade_limits,
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

#[derive(Debug, Clone, Copy)]
pub struct TradeLimit {
    pub minimum_size: Quantity,
    pub maximum_size: Quantity,
}

fn parse_trade_limits(raw: HashMap<String, RawTradeLimit>) -> Result<HashMap<Asset, TradeLimit>> {
    let mut trade_limits = HashMap::with_capacity(raw.len());

    for (symbol, limit) in raw {
        let minimum_size = Quantity::try_from(limit.minimum_size)
            .map_err(|error| eyre!("Invalid minimum trade size for {symbol}: {error}"))?;
        let maximum_size = Quantity::try_from(limit.maximum_size)
            .map_err(|error| eyre!("Invalid maximum trade size for {symbol}: {error}"))?;

        ensure!(
            minimum_size > Quantity::ZERO && minimum_size <= maximum_size,
            "Invalid trade limits for {symbol}: minimum={}, maximum={}",
            limit.minimum_size,
            limit.maximum_size
        );
        // TODO: fix this const being used outside file
        ensure!(
            minimum_size.atoms() % ORDER_SIZE_ATOM_STEP == 0
                && maximum_size.atoms() % ORDER_SIZE_ATOM_STEP == 0,
            "Trade limits for {symbol} must use at most six decimal places"
        );

        trade_limits.insert(
            Asset::from(symbol),
            TradeLimit {
                minimum_size,
                maximum_size,
            },
        );
    }

    Ok(trade_limits)
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
    trade_limit: HashMap<String, RawTradeLimit>,
}

#[derive(Debug, Deserialize)]
struct RawTradeLimit {
    minimum_size: f64,
    maximum_size: f64,
}
