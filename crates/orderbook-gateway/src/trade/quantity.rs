use crate::trade::price::PRICE_DECIMAL_PLACES;
use serde::{Deserialize, Serialize};
use std::{
    fmt::{self, Display, Formatter},
    num::NonZeroU64,
};

/// Order sizes may contain up to six decimal places.
///
/// This is our hardcoded arbitrary decision.
const ORDER_SIZE_DECIMAL_PLACES: u32 = 6;

/// Quantity stores both order sizes and quote costs (costs require size * price precision)
///
/// Multiplying a 6-decimal order size by a 2-decimal price can produce a quote cost with 8 decimal places.
/// This is an automatically derived result from the math conversions required to determine ATOMS_PER_UNIT
const QUANTITY_DECIMAL_PLACES: u32 = ORDER_SIZE_DECIMAL_PLACES + PRICE_DECIMAL_PLACES;

/// Number of atoms representing `1.0` in `Quantity` i.e. 10^8 or 100_000_000.
const ATOMS_PER_UNIT: u64 = 10_u64.pow(QUANTITY_DECIMAL_PLACES);

/// This factor converts between the scales `QUANTITY_DECIMAL_PLACES` and `ORDER_SIZE_DECIMAL_PLACES`.
///
/// Only quantities divisible by this factor convert to order sizes without precision loss.
const ORDER_SIZE_PRECISION_FACTOR: u64 =
    10_u64.pow(QUANTITY_DECIMAL_PLACES - ORDER_SIZE_DECIMAL_PLACES);

#[derive(Clone, Copy, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Quantity(u64);

impl Quantity {
    pub fn atoms(self) -> u64 {
        self.0
    }

    /// Whether this amount uses the precision allowed for an order size.
    pub fn valid_size(self) -> bool {
        self.0 % ORDER_SIZE_PRECISION_FACTOR == 0
    }
}

impl From<NonZeroU64> for Quantity {
    fn from(atoms: NonZeroU64) -> Self {
        Self(atoms.get())
    }
}

impl Display for Quantity {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0 as f64 / ATOMS_PER_UNIT as f64)
    }
}
