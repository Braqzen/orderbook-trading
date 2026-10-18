//! Publishes client executions to Redpanda.
//!
//! A crash after a match and before a successful produce can lose that execution.

use crate::trade::Instrument;
use eyre::{Result, eyre};
use rdkafka::{
    config::ClientConfig,
    producer::{FutureProducer, FutureRecord},
};
use std::time::Duration;
use tokio::{select, sync::mpsc::Receiver, time::sleep};
use tokio_util::sync::CancellationToken;
use tracing::error;
use uuid::Uuid;

pub struct OutboundExecution {
    pub client_id: Uuid,
    pub payload: String,
}

pub struct Publisher {
    producer: FutureProducer,
    inbound: Receiver<OutboundExecution>,
    instrument: Instrument,
}

impl Publisher {
    pub fn producer(brokers: &str) -> Result<FutureProducer> {
        ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .set("acks", "all")
            .create()
            .map_err(|error| eyre!(error))
    }

    pub fn new(
        instrument: Instrument,
        producer: FutureProducer,
        inbound: Receiver<OutboundExecution>,
    ) -> Self {
        Self {
            producer,
            inbound,
            instrument,
        }
    }

    pub async fn run(mut self, token: CancellationToken) -> Result<()> {
        loop {
            select! {
                _ = token.cancelled() => break,

                message = self.inbound.recv() => {
                    let Some(message) = message else {
                        break;
                    };

                    self.produce(message, &token).await;
                }
            }
        }

        Ok(())
    }

    async fn produce(&self, message: OutboundExecution, token: &CancellationToken) {
        let topic = format!("execution-{}", self.instrument);
        let key = message.client_id.to_string();

        loop {
            if token.is_cancelled() {
                return;
            }

            let record = FutureRecord::to(&topic)
                .key(&key)
                .payload(&message.payload);

            match self.producer.send(record, Duration::from_secs(5)).await {
                Ok(_) => return,
                Err((error, _)) => {
                    error!(
                        instrument = %self.instrument,
                        client_id = %message.client_id,
                        %error,
                        "Failed to produce execution"
                    );

                    select! {
                        _ = token.cancelled() => return,
                        _ = sleep(Duration::from_secs(1)) => {}
                    }
                }
            }
        }
    }
}
