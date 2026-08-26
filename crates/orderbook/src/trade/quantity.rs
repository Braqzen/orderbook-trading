use serde::Serialize;
use std::fmt::{self, Display, Formatter};
use std::ops::SubAssign;

const ATOMS_PER_UNIT: u64 = 100_000_000;
pub const ORDER_SIZE_ATOM_STEP: u64 = 100;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Quantity(u64);

impl Quantity {
    pub const ZERO: Self = Self(0);

    pub fn as_units(self) -> f64 {
        self.0 as f64 / ATOMS_PER_UNIT as f64
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
