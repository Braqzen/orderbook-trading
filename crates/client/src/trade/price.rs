use serde::Serialize;
use std::fmt::{self, Display, Formatter};

pub const CENTS_PER_UNIT: u64 = 100;

#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Price(u64);

impl Price {
    pub fn cents(self) -> u64 {
        self.0
    }
}

impl TryFrom<u64> for Price {
    type Error = String;

    fn try_from(cents: u64) -> Result<Self, Self::Error> {
        if cents == 0 {
            return Err("price must be positive".to_owned());
        }

        Ok(Self(cents))
    }
}

impl TryFrom<f64> for Price {
    type Error = String;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        let cents = value * CENTS_PER_UNIT as f64;

        if !cents.is_finite() || cents <= 0.0 || cents > u64::MAX as f64 {
            return Err("price must be a positive finite value".to_owned());
        }

        let rounded = cents.round();

        if (cents - rounded).abs() > 0.000_001 {
            return Err("price must have at most two decimal places".to_owned());
        }

        Ok(Self(rounded as u64))
    }
}

impl Display for Price {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}.{:02}",
            self.0 / CENTS_PER_UNIT,
            self.0 % CENTS_PER_UNIT
        )
    }
}
