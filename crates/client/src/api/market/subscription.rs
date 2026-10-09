//! Tracks instruments and whether they are subscribed to price events
//!
//! Additionally, randomly suggest actions to update its state / change client behaviour

use crate::{api::market::request::Operation, trade::Instrument};
use std::collections::HashSet;

pub struct Subscriptions {
    /// All available instruments the client may subscribe to
    ///
    /// Instruments are randomly selected per client on start
    available: Vec<Instrument>,
    /// Currently subscribed instruments
    subscribed: HashSet<Instrument>,
}

impl Subscriptions {
    pub fn new(available: Vec<Instrument>) -> Self {
        Self {
            subscribed: available.iter().cloned().collect(),
            available,
        }
    }

    pub fn available(&self) -> Vec<Instrument> {
        self.available.clone()
    }

    pub fn subscribed(&self) -> impl Iterator<Item = &Instrument> {
        self.subscribed.iter()
    }

    /// Returns iterator yielding instruments and if they are subscribed
    pub fn states(&self) -> impl Iterator<Item = (&Instrument, bool)> {
        self.available
            .iter()
            .map(|instrument| (instrument, self.subscribed.contains(instrument)))
    }

    pub fn update(&mut self, operation: Operation, instruments: &[Instrument]) {
        match operation {
            Operation::Subscribe => {
                self.subscribed.extend(instruments.iter().cloned());
            }
            Operation::Unsubscribe => {
                for instrument in instruments {
                    self.subscribed.remove(instrument);
                }
            }
        }
    }

    /// Randomly choose to subscribe, unsubscribe or fallback to no action
    pub fn random_action(&self) -> Option<(Operation, Vec<Instrument>)> {
        let can_subscribe = self.subscribed.len() < self.available.len();
        let can_unsubscribe = self.subscribed.len() > 1;

        let operation = match (can_subscribe, can_unsubscribe) {
            (true, true) => {
                if rand::random_bool(0.5) {
                    Operation::Subscribe
                } else {
                    Operation::Unsubscribe
                }
            }
            (true, false) => Operation::Subscribe,
            (false, true) => Operation::Unsubscribe,
            (false, false) => return None,
        };

        let candidates: Vec<_> = match operation {
            Operation::Subscribe => self
                .available
                .iter()
                .filter(|instrument| !self.subscribed.contains(*instrument))
                .cloned()
                .collect(),
            Operation::Unsubscribe => self.subscribed.iter().cloned().collect(),
        };

        if candidates.is_empty() {
            return None;
        }

        let instrument = candidates
            .get(rand::random_range(0..candidates.len()))?
            .clone();

        Some((operation, vec![instrument]))
    }
}
