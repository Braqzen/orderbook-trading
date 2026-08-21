use serde::Serialize;
use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Eq, Hash, PartialEq, Ord, PartialOrd, Serialize)]
pub struct Price(pub u64);

impl Display for Price {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}.{:02}", self.0 / 100, self.0 % 100)
    }
}

impl TryFrom<f64> for Price {
    type Error = String;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        let cents = value * 100.0;

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
