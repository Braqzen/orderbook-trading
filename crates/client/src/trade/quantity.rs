use std::fmt::{self, Display, Formatter};
use std::ops::{Add, AddAssign, Sub, SubAssign};

#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct Quantity(f64);

impl Quantity {
    pub const ZERO: Self = Self(0.0);

    pub fn value(self) -> f64 {
        self.0
    }

    pub fn mul_price(self, price: f64) -> Result<Self, String> {
        Self::try_from(self.0 * price)
    }
}

impl TryFrom<f64> for Quantity {
    type Error = String;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !value.is_finite() || value < 0.0 {
            return Err("quantity must be a non-negative finite value".to_owned());
        }

        Ok(Self(value))
    }
}

impl Add for Quantity {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for Quantity {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sub for Quantity {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign for Quantity {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl Display for Quantity {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.0)
    }
}
