use eyre::{Result, ensure};

pub struct PriceConfig {
    /// Initial value to start creating prices from
    pub start_price: f64,
    /// Upper bound the price is allowed to drift towards
    pub lower_limit: f64,
    /// Lower bound the price is allowed to drift towards
    pub upper_limit: f64,
}

impl PriceConfig {
    pub fn new(start_price: f64, upper_limit: f64, lower_limit: f64) -> Result<Self> {
        ensure!(
            lower_limit < start_price && start_price < upper_limit,
            "Price must satisfy lower_limit < start_price < upper_limit"
        );

        Ok(Self {
            start_price,
            lower_limit,
            upper_limit,
        })
    }
}
