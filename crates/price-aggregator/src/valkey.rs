//! Wrapper handling interaction with the cache, i.e. connecting and setting k-v pairs with a TTL

use eyre::{Result, eyre};
use redis::{AsyncCommands, Client, aio::ConnectionManager};

/// An instrument price is stored at `{PRICE_CACHE_KEY}:{instrument}`.
const PRICE_CACHE_KEY: &str = "price-aggregator";

#[derive(Clone)]
pub struct Cache {
    /// Object responsible for interacting with the cache
    connection: ConnectionManager,
    /// Quantity of time to mark an entry as expired
    ttl_seconds: u64,
}

impl Cache {
    pub async fn connect(url: &str, ttl_seconds: u64) -> Result<Self> {
        if ttl_seconds == 0 {
            return Err(eyre!("price TTL must be greater than zero"));
        }

        let client = Client::open(url)?;
        let connection = client.get_connection_manager().await?;

        Ok(Self {
            connection,
            ttl_seconds,
        })
    }

    pub async fn set(&self, instrument: &str, value: f64) -> Result<()> {
        let key = format!("{PRICE_CACHE_KEY}:{instrument}");
        let mut connection = self.connection.clone();

        let stored = match connection.set_ex(&key, value, self.ttl_seconds).await {
            Ok(()) => connection.publish(PRICE_CACHE_KEY, instrument).await,
            Err(error) => Err(error),
        };

        match stored {
            Ok(()) => Ok(()),
            Err(error) if error.is_unrecoverable_error() => {
                // Attempt to set the same price again to not miss an update
                connection
                    .set_ex::<_, _, ()>(&key, value, self.ttl_seconds)
                    .await?;
                connection
                    .publish(PRICE_CACHE_KEY, instrument)
                    .await
                    .map_err(Into::into)
            }
            Err(error) => Err(error.into()),
        }
    }
}
