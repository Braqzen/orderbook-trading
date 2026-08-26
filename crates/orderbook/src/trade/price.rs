use serde::Serialize;
use std::fmt::{self, Display, Formatter};

const CENTS_PER_UNIT: u64 = 100;

#[derive(Clone, Copy, Eq, Hash, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Price(u64);

impl Price {
    pub fn as_units(self) -> f64 {
        self.0 as f64 / CENTS_PER_UNIT as f64
    }
}

impl From<u64> for Price {
    fn from(cents: u64) -> Self {
        Self(cents)
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
