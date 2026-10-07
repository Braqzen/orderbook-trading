use crate::trade::units::{CENTS_PER_UNIT, Price, price::PRICE_DECIMAL_PLACES};
use serde::Serialize;
use std::{
    fmt::{self, Display, Formatter},
    ops::Mul,
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
pub const ORDER_SIZE_PRECISION_FACTOR: u64 =
    10_u64.pow(QUANTITY_DECIMAL_PLACES - ORDER_SIZE_DECIMAL_PLACES);

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Quantity(u64);

impl Quantity {
    pub const ZERO: Self = Self(0);

    /// Returns the internal integer representation.
    pub fn atoms(self) -> u64 {
        self.0
    }

    /// Converts the internal representation to its decimal equivalent.
    pub fn to_decimals(self) -> u64 {
        self.0 / ORDER_SIZE_PRECISION_FACTOR
    }

    /// Expresses an amount in asset units rather than the smallest denomination used internally.
    ///
    /// An internal value of `250_000_000` becomes `2.5` representing 2.5 BTC, 2.5 USD etc. depending on asset
    pub fn as_units(self) -> f64 {
        self.0 as f64 / ATOMS_PER_UNIT as f64
    }

    pub fn checked_add(self, rhs: Self) -> Result<Self, String> {
        self.0
            .checked_add(rhs.0)
            .map(Self)
            .ok_or_else(|| "quantity overflow".to_owned())
    }

    pub fn checked_sub(self, rhs: Self) -> Result<Self, String> {
        self.0
            .checked_sub(rhs.0)
            .map(Self)
            .ok_or_else(|| "quantity underflow".to_owned())
    }
}

impl From<u64> for Quantity {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

impl TryFrom<f64> for Quantity {
    type Error = String;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !value.is_finite() || value < 0.0 {
            return Err("quantity must be a non-negative finite value".to_owned());
        }

        let scaled = value * ATOMS_PER_UNIT as f64;
        if scaled > u64::MAX as f64 {
            return Err("quantity exceeds supported range".to_owned());
        }

        Ok(Self(scaled.round() as u64))
    }
}

impl Mul<Price> for Quantity {
    type Output = Result<Self, String>;

    fn mul(self, price: Price) -> Self::Output {
        (self.0 / CENTS_PER_UNIT)
            .checked_mul(price.cents())
            .map(Self)
            .ok_or_else(|| "quantity multiplication overflow".to_owned())
    }
}

impl Display for Quantity {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.as_units())
    }
}
