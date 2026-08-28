use crate::trade::{CENTS_PER_UNIT, Price};
use serde::Serialize;
use std::fmt::{self, Display, Formatter};
use std::ops::Mul;

// Fixed-point scale: 1 unit = 10^8 atoms (8 decimal places).
const ATOMS_PER_UNIT: u64 = 100_000_000;
/// Order sizes use six decimal places so cent-priced trades produce whole quote atoms.
pub const ORDER_SIZE_ATOM_STEP: u64 = 100;

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Quantity(u64);

impl Quantity {
    pub const ZERO: Self = Self(0);

    pub fn atoms(self) -> u64 {
        self.0
    }

    pub fn to_decimals(self) -> u64 {
        self.0 / ORDER_SIZE_ATOM_STEP
    }

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
        if self.0 % CENTS_PER_UNIT != 0 {
            return Err("quantity cannot be priced exactly".to_owned());
        }

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
