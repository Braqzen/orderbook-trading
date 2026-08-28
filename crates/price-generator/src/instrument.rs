use eyre::{Result, eyre};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

/// Represents a trading pair e.g. TSLA-USD, BTC-USD...
///
/// Split by the hyphen into 2 assets, the base and then the pair
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Instrument {
    base: Asset,
    quote: Asset,
}

impl TryFrom<&str> for Instrument {
    type Error = eyre::Report;

    fn try_from(value: &str) -> Result<Self> {
        // Uninterested in proper validation. Assume correct input
        let mut parts = value.split('-');
        let base = parts
            .next()
            .ok_or_else(|| eyre!("Invalid instrument: {value}"))?;
        let quote = parts
            .next()
            .ok_or_else(|| eyre!("Invalid instrument: {value}"))?;

        Ok(Self {
            base: Asset(base.to_owned()),
            quote: Asset(quote.to_owned()),
        })
    }
}

impl Display for Instrument {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}-{}", self.base.0, self.quote.0)
    }
}

/// Wrapper type to enrich meaning beyond a simple String
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Asset(String);
