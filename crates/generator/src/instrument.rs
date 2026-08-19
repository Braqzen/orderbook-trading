use eyre::{Result, ensure, eyre};
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Instrument {
    base: Asset,
    quote: Asset,
}

impl TryFrom<&str> for Instrument {
    type Error = eyre::Report;

    fn try_from(value: &str) -> Result<Self> {
        let (base, quote) = value
            .split_once('-')
            .ok_or_else(|| eyre!("Invalid instrument: {value}"))?;

        ensure!(
            !base.is_empty() && !quote.is_empty() && !quote.contains('-'),
            "Invalid instrument: {value}"
        );

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

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Asset(String);
