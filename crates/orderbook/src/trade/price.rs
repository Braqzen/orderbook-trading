use serde::Serialize;
use std::{
    fmt::{self, Display, Formatter},
    num::NonZeroU64,
};

/// Price is in cents which contains 2 decimal places.
pub const PRICE_DECIMAL_PLACES: u32 = 2;
/// Number of cents representing 1 price unit ($1 = 10^2 or 100 cents).
const CENTS_PER_UNIT: u64 = 10_u64.pow(PRICE_DECIMAL_PLACES);

#[derive(Clone, Copy, Eq, Hash, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Price(u64);

impl Price {
    pub fn as_units(self) -> f64 {
        self.0 as f64 / CENTS_PER_UNIT as f64
    }
}

impl From<NonZeroU64> for Price {
    fn from(cents: NonZeroU64) -> Self {
        Self(cents.get())
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
