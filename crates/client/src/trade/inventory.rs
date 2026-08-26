use crate::trade::{Asset, Order, Price, Quantity};
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

    pub fn reserved(&self, asset: &Asset) -> Option<Quantity> {
        self.reserved.get(asset).copied()
    }

    pub fn assets(&self) -> Vec<Asset> {
        self.available.keys().cloned().collect()
    }

    pub fn reserve(&mut self, asset: &Asset, amount: Quantity) -> Result<(), String> {
        if amount <= Quantity::ZERO {
            return Err("reserve amount must be a positive finite value".to_owned());
        }

        let available = match self.available.get(asset).copied() {
            Some(available) => available,
            None => return Err("asset not in inventory".to_owned()),
        };

        if available < amount {
            return Err("insufficient available inventory".to_owned());
        }

        let available = available.checked_sub(amount)?;
        let reserved = self
            .reserved
            .get(asset)
            .copied()
            .unwrap_or(Quantity::ZERO)
            .checked_add(amount)?;

        self.available.insert(asset.clone(), available);
        self.reserved.insert(asset.clone(), reserved);
        Ok(())
    }

    pub fn release(&mut self, asset: &Asset, amount: Quantity) -> Result<(), String> {
        if amount <= Quantity::ZERO {
            return Err("release amount must be a positive finite value".to_owned());
        }

        let reserved = match self.reserved.get(asset).copied() {
            Some(reserved) => reserved,
            None => return Err("asset not in reserved inventory".to_owned()),
        };

        if reserved < amount {
            return Err("insufficient reserved inventory".to_owned());
        }

        if !self.available.contains_key(asset) {
            return Err("asset not in available inventory".to_owned());
        }

        let available = match self.available.get(asset).copied() {
            Some(available) => available.checked_add(amount)?,
            None => return Err("asset not in available inventory".to_owned()),
        };
        let reserved = reserved.checked_sub(amount)?;

        self.reserved.insert(asset.clone(), reserved);
        self.available.insert(asset.clone(), available);
        Ok(())
    }

    pub fn apply_buy(
        &mut self,
        order: &Order,
        fill_size: Quantity,
        fill_price: Price,
    ) -> Result<(), String> {
        let base = order.instrument.base();
        let quote = order.instrument.quote();
        let limit_price = order.price;

        if fill_size <= Quantity::ZERO {
            return Err("fill size must be a positive finite value".to_owned());
        }

        // Check how much was reserved when the order was placed
        let quote_reserved = (fill_size * limit_price)?;
        // And how much was actually fulfilled
        let quote_paid = (fill_size * fill_price)?;

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

        let released_quote = quote_reserved.checked_sub(quote_paid)?;
        let reserved_quote = reserved_quote.checked_sub(quote_reserved)?;
        let available_quote = self
            .available
            .get(quote)
            .copied()
            .unwrap_or(Quantity::ZERO)
            .checked_add(released_quote)?;
        let available_base = self
            .available
            .get(base)
            .copied()
            .unwrap_or(Quantity::ZERO)
            .checked_add(fill_size)?;

        self.reserved.insert(quote.clone(), reserved_quote);
        self.available.insert(quote.clone(), available_quote);
        self.available.insert(base.clone(), available_base);
        Ok(())
    }

    pub fn apply_sell(
        &mut self,
        order: &Order,
        fill_size: Quantity,
        fill_price: Price,
    ) -> Result<(), String> {
        let base = order.instrument.base();
        let quote = order.instrument.quote();

        if fill_size <= Quantity::ZERO {
            return Err("fill size must be a positive finite value".to_owned());
        }

        let quote_received = (fill_size * fill_price)?;

        let reserved_base = match self.reserved.get(base).copied() {
            Some(reserved_base) => reserved_base,
            None => return Err("asset not in reserved inventory".to_owned()),
        };

        if reserved_base < fill_size {
            return Err("insufficient reserved inventory".to_owned());
        }

        let reserved_base = reserved_base.checked_sub(fill_size)?;
        let available_quote = self
            .available
            .get(quote)
            .copied()
            .unwrap_or(Quantity::ZERO)
            .checked_add(quote_received)?;

        self.reserved.insert(base.clone(), reserved_base);
        self.available.insert(quote.clone(), available_quote);
        Ok(())
    }
}
