use crate::{
    instrument::Instrument,
    simulation::{config::PriceConfig, price::Price},
};
use rand::{random_bool, random_range};

/// Percentage of range treated as a zone that is too close to the limit
///
/// The closer a value is to the limit the smaller the zone between that value and the limit.
/// When that zone is some percentage of the whole it is considered too close to the limit.
const EDGE_ZONE_RATIO: f64 = 0.1;

pub struct PriceManager {
    /// The symbol associated with the generated price
    instrument: Instrument,
    /// A mutable value representing the latest generated price
    current_price: f64,
    /// Upper bound the price is allowed to drift towards
    upper_limit: f64,
    /// Lower bound the price is allowed to drift towards
    lower_limit: f64,
}

impl PriceManager {
    pub fn new(instrument: Instrument, config: PriceConfig) -> Self {
        Self {
            instrument,
            current_price: config.start_price,
            upper_limit: config.upper_limit,
            lower_limit: config.lower_limit,
        }
    }

    pub fn next_price(&mut self) -> Price {
        // Select how much the current price should change by: 1%, 2%, ..
        let magnitude = match random_range(0_u8..100) {
            0..=75 => random_range(0.0..0.001),
            76..=90 => random_range(0.001..0.01),
            _ => random_range(0.01..0.05),
        };

        // Figure out the range of acceptable values
        let range = self.upper_limit - self.lower_limit;
        // From that range calculate a range which is considered near the edge of the limits
        let edge_zone = range * EDGE_ZONE_RATIO;

        // Figure out how close we are to the limit
        let lower_proximity =
            ((edge_zone - (self.current_price - self.lower_limit)) / edge_zone).clamp(0.0, 1.0);
        let upper_proximity =
            ((edge_zone - (self.upper_limit - self.current_price)) / edge_zone).clamp(0.0, 1.0);

        // Push the price back toward the middle as it approaches either limit.
        let increase_probability = 0.5 + (lower_proximity - upper_proximity) * 0.5;
        let direction = if random_bool(increase_probability) {
            1.0
        } else {
            -1.0
        };

        // Create a new price value
        let new_value = self.current_price * (1.0 + direction * magnitude);

        // If outside limits then use the difference between limit and value to reflect it back inside range
        let bounded = if new_value <= self.lower_limit {
            self.lower_limit + (self.lower_limit - new_value)
        } else if new_value >= self.upper_limit {
            self.upper_limit - (new_value - self.upper_limit)
        } else {
            new_value
        };

        // Round to 2 decimal places
        let rounded = ((bounded * 100.0).round() / 100.0)
            .clamp(self.lower_limit + 0.01, self.upper_limit - 0.01);

        // Make sure to update the price after each generation
        self.current_price = rounded;

        Price::new(self.instrument.clone(), rounded)
    }
}
