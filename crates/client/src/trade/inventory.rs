use crate::trade::{Asset, Quantity};
use std::collections::HashMap;

pub struct Inventory {
    available: HashMap<Asset, Quantity>,
    reserved: HashMap<Asset, Quantity>,
}

impl Inventory {
    pub fn new(values: Vec<(String, Quantity)>) -> Self {
        let available = values
            .into_iter()
            .map(|(asset, amount)| (Asset::from(asset), amount))
            .collect();

        Self {
            available,
            reserved: HashMap::new(),
        }
    }

    pub fn available(&self, asset: &Asset) -> Option<Quantity> {
        self.available.get(asset).copied()
    }

    pub fn reserve(&mut self, asset: &Asset, amount: Quantity) -> Result<(), String> {
        if amount <= Quantity::ZERO {
            return Err("reserve amount must be a positive finite value".to_owned());
        }

        let available = match self.available.get_mut(asset) {
            Some(available) => available,
            None => return Err("asset not in inventory".to_owned()),
        };

        if *available < amount {
            return Err("insufficient available inventory".to_owned());
        }

        *available -= amount;

        match self.reserved.get_mut(asset) {
            Some(reserved) => *reserved += amount,
            None => {
                self.reserved.insert(asset.clone(), amount);
            }
        }

        Ok(())
    }

    pub fn apply_buy(
        &mut self,
        base: &Asset,
        quote: &Asset,
        fill_size: Quantity,
        fill_price: f64,
        limit_price: f64,
    ) -> Result<(), String> {
        if fill_size <= Quantity::ZERO {
            return Err("fill size must be a positive finite value".to_owned());
        }

        if !fill_price.is_finite() || fill_price <= 0.0 {
            return Err("fill price must be a positive finite value".to_owned());
        }

        if !limit_price.is_finite() || limit_price <= 0.0 {
            return Err("limit price must be a positive finite value".to_owned());
        }

        // Check how much was reserved when the order was placed
        let quote_reserved = fill_size.mul_price(limit_price)?;
        // And how much was actually fulfilled
        let quote_paid = fill_size.mul_price(fill_price)?;

        let reserved_quote = match self.reserved.get(quote).copied() {
            Some(reserved_quote) => reserved_quote,
            None => return Err("asset not in reserved inventory".to_owned()),
        };

        if reserved_quote < quote_reserved {
            return Err("insufficient reserved inventory".to_owned());
        }

        if quote_reserved < quote_paid {
            return Err("fill exceeds reserved inventory".to_owned());
        }

        match self.reserved.get_mut(quote) {
            Some(reserved) => *reserved -= quote_reserved,
            None => return Err("asset not in reserved inventory".to_owned()),
        }

        match self.available.get_mut(quote) {
            Some(available) => *available += quote_reserved - quote_paid,
            None => {
                self.available
                    .insert(quote.clone(), quote_reserved - quote_paid);
            }
        }

        match self.available.get_mut(base) {
            Some(available) => *available += fill_size,
            None => {
                self.available.insert(base.clone(), fill_size);
            }
        }

        Ok(())
    }

    pub fn apply_sell(
        &mut self,
        base: &Asset,
        quote: &Asset,
        fill_size: Quantity,
        fill_price: f64,
    ) -> Result<(), String> {
        if fill_size <= Quantity::ZERO {
            return Err("fill size must be a positive finite value".to_owned());
        }

        if !fill_price.is_finite() || fill_price <= 0.0 {
            return Err("fill price must be a positive finite value".to_owned());
        }

        let quote_received = fill_size.mul_price(fill_price)?;

        let reserved_base = match self.reserved.get(base).copied() {
            Some(reserved_base) => reserved_base,
            None => return Err("asset not in reserved inventory".to_owned()),
        };

        if reserved_base < fill_size {
            return Err("insufficient reserved inventory".to_owned());
        }

        match self.reserved.get_mut(base) {
            Some(reserved) => *reserved -= fill_size,
            None => return Err("asset not in reserved inventory".to_owned()),
        }

        match self.available.get_mut(quote) {
            Some(available) => *available += quote_received,
            None => {
                self.available.insert(quote.clone(), quote_received);
            }
        }

        Ok(())
    }
}
