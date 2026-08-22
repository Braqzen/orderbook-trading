use serde::Serialize;
use std::fmt::{self, Display, Formatter};

#[derive(Serialize)]
pub struct ClientRequest {
    pub op: Operation,
    pub instruction: Instruction,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Subscribe,
}

impl ClientRequest {
    pub fn subscribe(instruments: Vec<String>) -> Self {
        Self {
            op: Operation::Subscribe,
            instruction: Instruction::Instruments { instruments },
        }
    }
}

impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Subscribe => "subscribe",
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
