use crate::trade::Asset;
use std::collections::HashMap;

pub struct Inventory {
    available: HashMap<Asset, f64>,
    reserved: HashMap<Asset, f64>,
}

impl Inventory {
    pub fn new(values: Vec<(String, f64)>) -> Self {
        let available = values
            .into_iter()
            .map(|(asset, amount)| (Asset::from(asset), amount))
            .collect();

        Self {
            available,
            reserved: HashMap::new(),
        }
    }

    pub fn available(&self, asset: &Asset) -> Option<f64> {
        self.available.get(asset).copied()
    }
}
