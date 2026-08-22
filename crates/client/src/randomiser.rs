use crate::{
    config::Config,
    trade::{Inventory, Quantity},
};
use eyre::{Result, ensure, eyre};
use rand::seq::SliceRandom;

const USD_SYMBOL: &str = "USD";
const MIN_ASSETS: usize = 1;
const MAX_ASSETS: usize = 4;

const MIN_INSTRUMENTS: usize = 1;
const MAX_INSTRUMENTS: usize = 4;

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

    pub fn inventory(&self) -> Result<Inventory> {
        let (mut usd, mut assets): (Vec<_>, Vec<_>) = self
            .config
            .inventory
            .clone()
            .into_iter()
            .partition(|entry| entry.symbol == USD_SYMBOL);

        assets.shuffle(&mut rand::rng());
        assets.truncate(rand::random_range(
            MIN_ASSETS..=MAX_ASSETS.min(assets.len()),
        ));
        usd.extend(assets);

        let values = usd
            .into_iter()
            .map(|entry| {
                let amount = rand::random_range(entry.lower_limit..=entry.upper_limit);
                let amount = Quantity::try_from(amount)
                    .map_err(|error| eyre!("Invalid amount for {}: {error}", entry.symbol))?;

                Ok((entry.symbol, amount))
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Inventory::new(values))
    }

    pub fn instruments(&self) -> Vec<String> {
        let mut instruments = self.config.instruments.clone();
        instruments.shuffle(&mut rand::rng());
        instruments.truncate(rand::random_range(
            MIN_INSTRUMENTS..=MAX_INSTRUMENTS.min(instruments.len()),
        ));

        instruments
    }
}
