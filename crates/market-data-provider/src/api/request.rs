use serde::Deserialize;
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Deserialize)]
pub struct ClientRequest {
    pub op: Operation,
    pub instruction: Instruction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    Subscribe,
}

impl Operation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Subscribe => "subscribe",
        }
    }
}

impl TryFrom<&str> for Operation {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "subscribe" => Ok(Self::Subscribe),
            _ => Err(format!("unsupported operation: {value}")),
        }
    }
}

impl Display for Operation {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Instruction {
    Instruments { instruments: Vec<String> },
}
