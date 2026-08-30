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

        // Compose instrument BASE-QUOTE to ensure both are in the client inventory
        // otherwise cannot trade
        for instrument in config.instruments.keys() {
            ensure!(
                inventory_assets.contains(instrument.base().as_str())
                    && inventory_assets.contains(instrument.quote().as_str()),
                "Config inventory must contain both assets for {instrument}"
            );
        }

        Ok(Self { config })
    }

    pub fn randomise(&self) -> Result<(Vec<Instrument>, Inventory)> {
        let mut instruments: Vec<Instrument> = self.config.instruments.keys().cloned().collect();
        instruments.shuffle(&mut rand::rng());
        let instrument_count = rand::random_range(MIN_INSTRUMENTS..=self.config.instruments.len());
        instruments.truncate(instrument_count);

        let mut selected_assets = HashSet::from([USD_SYMBOL.to_owned()]);

        for instrument in instruments.iter() {
            selected_assets.insert(instrument.base().as_str().to_owned());
            selected_assets.insert(instrument.quote().as_str().to_owned());
        }

        let entries = self
            .config
            .inventory
            .clone()
            .into_iter()
            .filter(|entry| selected_assets.contains(&entry.symbol));

        let values = entries
            .map(|entry| {
                let amount = rand::random_range(entry.lower_limit..=entry.upper_limit);
                let amount = Quantity::try_from(amount)
                    .map_err(|error| eyre!("Invalid amount for {}: {error}", entry.symbol))?;

                Ok((entry.symbol, amount))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok((instruments, Inventory::new(values)))
    }
}
