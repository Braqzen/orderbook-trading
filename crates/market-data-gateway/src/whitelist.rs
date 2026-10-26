//! Instrument whitelist loaded from JSON at startup.
//!
//! Clients may subscribe only to instruments listed in the config file.

use eyre::{Result, eyre};
use serde::Deserialize;
use std::{collections::HashSet, fs, path::Path};

#[derive(Debug, Deserialize)]
struct InstrumentsFile {
    instruments: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct InstrumentWhitelist {
    instruments: HashSet<String>,
}

impl InstrumentWhitelist {
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let contents = fs::read_to_string(path.as_ref())?;

        let file: InstrumentsFile = serde_json::from_str(&contents)?;

        if file.instruments.is_empty() {
            return Err(eyre!(
                "instruments config must list at least one instrument"
            ));
        }

        let instruments: HashSet<String> = file.instruments.iter().cloned().collect();

        Ok(Self { instruments })
    }

    pub fn contains(&self, instrument: &str) -> bool {
        self.instruments.contains(instrument)
    }
}
