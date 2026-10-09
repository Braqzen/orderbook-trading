//! Requests the market data provider accepts
//!
//! An example operation is subscribing to specific instrument price updates

use crate::trade::Instrument;
use serde::{Deserialize, Serialize};
use std::fmt::{self, Display, Formatter};

#[derive(Serialize)]
pub struct ClientRequest {
    pub op: Operation,
    pub instruction: Instruction,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Subscribe,
    Unsubscribe,
}

impl ClientRequest {
    pub fn subscribe(instruments: Vec<Instrument>) -> Self {
        Self {
            op: Operation::Subscribe,
            instruction: Instruction::Instruments {
                instruments: instruments.iter().map(ToString::to_string).collect(),
            },
        }
    }

    pub fn unsubscribe(instruments: Vec<Instrument>) -> Self {
        Self {
            op: Operation::Unsubscribe,
            instruction: Instruction::Instruments {
                instruments: instruments.iter().map(ToString::to_string).collect(),
            },
        }
    }
}

impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Subscribe => "subscribe",
            Self::Unsubscribe => "unsubscribe",
        }
    }
}

impl Display for Operation {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Serialize)]
#[serde(untagged)]
pub enum Instruction {
    Instruments { instruments: Vec<String> },
}
