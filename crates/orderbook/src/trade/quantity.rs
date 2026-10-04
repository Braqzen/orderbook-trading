use crate::trade::price::PRICE_DECIMAL_PLACES;
use serde::Serialize;
use std::{
    fmt::{self, Display, Formatter},
    ops::SubAssign,
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

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Quantity(u64);

impl Quantity {
    pub const ZERO: Self = Self(0);

    /// Expresses an amount in asset units rather than the smallest denomination used internally.
    ///
    /// An internal value of `250_000_000` becomes `2.5` representing 2.5 BTC, 2.5 USD etc. depending on asset
    pub fn as_units(self) -> f64 {
        self.0 as f64 / ATOMS_PER_UNIT as f64
    }

    /// Whether this amount uses the precision allowed for an order size.
    pub fn valid_size(self) -> bool {
        self.0 % ORDER_SIZE_PRECISION_FACTOR == 0
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, String> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or_else(|| "quantity overflow".to_owned())
    }
}

impl SubAssign for Quantity {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl From<u64> for Quantity {
    fn from(atoms: u64) -> Self {
        Self(atoms)
    }
}

impl Display for Quantity {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.as_units())
    }
}
