//! Wrapper handling the client's execution sequence in the existing cache.

use crate::trade::Instrument;
use eyre::{Result, eyre};
use redis::{AsyncCommands, Client, aio::ConnectionManager};
use uuid::Uuid;

/// A client execution sequence is stored at `{SEQUENCE_KEY}:{client}:{instrument}`.
const SEQUENCE_KEY: &str = "orderbook-client";

#[derive(Clone)]
pub struct SequenceCache {
    connection: ConnectionManager,
}

impl SequenceCache {
    pub async fn connect(url: &str) -> Result<Self> {
        let client = Client::open(url)?;
        let connection = client.get_connection_manager().await?;

        Ok(Self { connection })
    }

    pub async fn last_exec_seq(&self, client_id: Uuid, instrument: &Instrument) -> Result<u64> {
        let key = self.key(client_id, instrument);
        let mut connection = self.connection.clone();
        let value: Option<String> = connection.get(&key).await?;

        match value {
            None => Ok(0),
            Some(value) => value
                .parse::<u64>()
                .map_err(|error| eyre!("invalid execution sequence at {key}: {error}")),
        }
    }

    pub async fn store(
        &self,
        client_id: Uuid,
        instrument: &Instrument,
        exec_seq: u64,
    ) -> Result<()> {
        let key = self.key(client_id, instrument);
        let mut connection = self.connection.clone();

        match connection.set::<_, _, ()>(&key, exec_seq.to_string()).await {
            Ok(()) => Ok(()),
            Err(error) if error.is_unrecoverable_error() => connection
                .set::<_, _, ()>(&key, exec_seq.to_string())
                .await
                .map_err(Into::into),
            Err(error) => Err(error.into()),
        }
    }

    fn key(&self, client_id: Uuid, instrument: &Instrument) -> String {
        format!("{SEQUENCE_KEY}:{client_id}:{instrument}")
    }
}
