//! Process environment for the orderbook.

use eyre::Result;

pub struct Environment {
    /// Bind to this socket to accept gateway order submissions
    pub grpc: String,
    /// The only instrument this orderbook processes
    pub instrument: String,
    /// Redpanda brokers executions are published to
    pub redpanda_brokers: String,
}

impl Environment {
    pub fn load() -> Result<Self> {
        Ok(Self {
            grpc: std::env::var("GRPC")?,
            instrument: std::env::var("INSTRUMENT")?,
            redpanda_brokers: std::env::var("REDPANDA_BROKERS")?,
        })
    }
}
