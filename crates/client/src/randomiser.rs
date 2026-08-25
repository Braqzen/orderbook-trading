use crate::{
    config::Config,
    trade::{Instrument, Inventory, Quantity},
};
use eyre::{Result, ensure, eyre};
use rand::seq::SliceRandom;
use std::collections::HashSet;

const USD_SYMBOL: &str = "USD";
const MIN_ASSETS: usize = 1;
const MIN_INSTRUMENTS: usize = 1;

pub struct Randomiser {
    config: Config,
}

impl Randomiser {
    pub fn new(config: Config) -> Result<Self> {
        let (usd, assets): (Vec<_>, Vec<_>) = config
            .inventory
            .iter()
            .partition(|entry| entry.symbol == USD_SYMBOL);

        ensure!(
            usd.len() == 1,
            "Config inventory must contain exactly one USD entry"
        );
        ensure!(
            assets.len() >= MIN_ASSETS,
            "Config inventory must contain at least one asset"
        );
        ensure!(
            config.instruments.len() >= MIN_INSTRUMENTS,
            "Config must contain at least one instrument"
        );

        let inventory_assets: HashSet<&str> = config
            .inventory
            .iter()
            .map(|entry| entry.symbol.as_str())
            .collect();

        for instrument in config.instruments.keys() {
            ensure!(
                inventory_assets.contains(instrument.base().as_str())
                    && inventory_assets.contains(instrument.quote().as_str()),
                "Config inventory must contain both assets for {instrument}"
            );
        }

        for entry in &config.inventory {
            ensure!(
                entry.lower_limit.is_finite()
                    && entry.upper_limit.is_finite()
                    && entry.lower_limit <= entry.upper_limit,
                "Invalid limits for {}: lower={}, upper={}",
                entry.symbol,
                entry.lower_limit,
                entry.upper_limit
            );
        }

        Ok(Self { config })
    }

    pub fn inventory(&self, instruments: &[Instrument]) -> Result<Inventory> {
        let mut selected_assets = HashSet::from([USD_SYMBOL]);

        for instrument in instruments {
            selected_assets.insert(instrument.base().as_str());
            selected_assets.insert(instrument.quote().as_str());
        }

        let entries = self
            .config
            .inventory
            .clone()
            .into_iter()
            .filter(|entry| selected_assets.contains(entry.symbol.as_str()));

        let values = entries
            .map(|entry| {
                let amount = rand::random_range(entry.lower_limit..=entry.upper_limit);
                let amount = Quantity::try_from(amount)
                    .map_err(|error| eyre!("Invalid amount for {}: {error}", entry.symbol))?;

                Ok((entry.symbol, amount))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Inventory::new(values))
    }

    pub fn instruments(&self) -> Vec<Instrument> {
        let mut instruments: Vec<Instrument> = self.config.instruments.keys().cloned().collect();
        instruments.shuffle(&mut rand::rng());
        let instrument_count = rand::random_range(MIN_INSTRUMENTS..=self.config.instruments.len());
        instruments.truncate(instrument_count);

        instruments
    }
}
