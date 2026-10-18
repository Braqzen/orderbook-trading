//! Reads one client's executions from Redpanda and forwards them on the session channel.
//!
//! The reader starts at the beginning of every partition because Redpanda cannot seek by client.
//! Executions at or below `last_exec_seq` are not forwarded. The same read then forwards executions above that cursor.

use crate::{api::Response, trade::Instrument};
use eyre::{Result, eyre};
use futures_util::StreamExt;
use rdkafka::{
    Message, Offset,
    config::ClientConfig,
    consumer::{Consumer, StreamConsumer},
    message::BorrowedMessage,
    topic_partition_list::TopicPartitionList,
};
use std::time::Duration;
use tokio::{select, sync::mpsc::Sender};
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};
use uuid::Uuid;

/// Partition count of `execution-{instrument}`. All of them are scanned because the client key can land on any one.
const PARTITIONS: i32 = 4;

#[derive(Clone)]
pub struct ExecutionReader {
    /// Redpanda brokers
    brokers: String,
    /// Instrument whose `execution-{instrument}` topic is read
    instrument: Instrument,
}

impl ExecutionReader {
    pub fn new(brokers: String, instrument: Instrument) -> Self {
        Self {
            brokers,
            instrument,
        }
    }

    pub async fn run(
        &self,
        client_id: Uuid,
        mut last_exec_seq: u64,
        outbound: Sender<Response>,
        token: CancellationToken,
    ) -> Result<()> {
        let consumer: StreamConsumer = ClientConfig::new()
            .set("bootstrap.servers", &self.brokers)
            .set("enable.auto.commit", "false")
            .set("enable.partition.eof", "false")
            .set(
                "group.id",
                format!("orderbook-gateway-{client_id}-{}", Uuid::new_v4()),
            )
            .create()
            .map_err(|error| eyre!(error))?;

        let topic = format!("execution-{}", self.instrument);
        self.assign(&consumer, &topic)?;

        let mut stream = consumer.stream();

        loop {
            select! {
                _ = token.cancelled() => break,

                // Orderbook sent new msg to redpanda
                message = stream.next() => {
                    let Some(message) = message else {
                        break;
                    };

                    let message = match message {
                        Ok(message) => message,
                        Err(error) => {
                            error!(instrument = %self.instrument, %client_id, %error, "Failed to read execution");
                            continue;
                        }
                    };

                    if !self.same_client(&message, client_id) {
                        continue;
                    }

                    let Some(payload) = message.payload() else {
                        warn!(instrument = %self.instrument, %client_id, "Execution had no payload");
                        continue;
                    };

                    let response = match serde_json::from_slice::<Response>(payload) {
                        Ok(response) => response,
                        Err(error) => {
                            error!(instrument = %self.instrument, %client_id, %error, "Failed to deserialize execution");
                            continue;
                        }
                    };

                    let Some(exec_seq) = response.exec_seq() else {
                        continue;
                    };

                    if exec_seq <= last_exec_seq {
                        continue;
                    }

                    match outbound.try_send(response) {
                        Ok(()) => last_exec_seq = exec_seq,
                        Err(_) => {
                            warn!(instrument = %self.instrument, %client_id, "Client outbound queue closed");
                            break;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn assign(&self, consumer: &StreamConsumer, topic: &str) -> Result<()> {
        let mut partitions = TopicPartitionList::new();

        for partition in 0..PARTITIONS {
            partitions
                .add_partition_offset(topic, partition, Offset::Beginning)
                .map_err(|error| eyre!(error))?;
        }

        consumer.assign(&partitions).map_err(|error| eyre!(error))?;

        for partition in 0..PARTITIONS {
            consumer
                .seek(topic, partition, Offset::Beginning, Duration::from_secs(5))
                .map_err(|error| eyre!(error))?;
        }

        Ok(())
    }

    fn same_client(&self, message: &BorrowedMessage<'_>, client_id: Uuid) -> bool {
        let Some(key) = message.key() else {
            return false;
        };

        key == client_id.to_string().as_bytes()
    }
}
