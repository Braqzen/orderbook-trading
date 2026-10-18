use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectionReason {
    InvalidInstrument,
    InvalidOrderSize,
}
