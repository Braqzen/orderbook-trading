use crate::{
    api::{
        Response, WsUrl,
        orderbook::{RequestMetadata, connection::Connection},
    },
    trade::Instrument,
};
use eyre::{Result, ensure, eyre};
use std::collections::HashMap;
use tokio::{
    select,
    sync::mpsc::{self, Receiver, Sender, error::TrySendError},
    task::{JoinError, JoinSet},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};
use uuid::Uuid;

pub struct OrderBook {
    client_id: Uuid,
    /// Track which instrument is associated with an orderbook
    subscriptions: Vec<(Instrument, WsUrl)>,
    /// Requests received from the engine to send to an orderbook
    order_receiver_channel: Receiver<RequestMetadata>,
    /// Receive responses from an orderbook and forward to engine
    response_sender_channel: Sender<Response>,
}

impl OrderBook {
    pub fn new(
        client_id: Uuid,
        subscriptions: Vec<Instrument>,
        instruments: HashMap<Instrument, WsUrl>,
        order_receiver_channel: Receiver<RequestMetadata>,
        response_sender_channel: Sender<Response>,
    ) -> Result<Self> {
        let mut resolved = Vec::with_capacity(subscriptions.len());

        for instrument in subscriptions {
            let url = instruments.get(&instrument).cloned().ok_or_else(|| {
                eyre!("Missing orderbook URL for subscribed instrument {instrument}")
            })?;
            resolved.push((instrument, url));
        }

        ensure!(
            !resolved.is_empty(),
            "Must subscribe to at least one instrument with an orderbook"
        );

        Ok(Self {
            client_id,
            subscriptions: resolved,
            order_receiver_channel,
            response_sender_channel,
        })
    }

    pub async fn run(mut self, token: CancellationToken) -> Result<()> {
        let mut order_senders = HashMap::new();
        let mut connections = JoinSet::new();

        // Create a connection to each orderbook and forward requests to them based on which instrument they take
        // Connections will send requests and receive orderbook responses
        // Responses are sent back to the engine for final accounting
        for (instrument, url) in self.subscriptions {
            let (order_sender_channel, order_receiver_channel) = mpsc::channel(128);
            order_senders.insert(instrument.clone(), order_sender_channel);

            let connection = Connection::new(
                self.client_id,
                instrument,
                url,
                order_receiver_channel,
                self.response_sender_channel.clone(),
            );
            let connection_token = token.child_token();
            connections.spawn(connection.run(connection_token));
        }

        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                // Engine sent request, forward it to the correct orderbook connection for processing
                request = self.order_receiver_channel.recv() => {
                    let Some(request) = request else {
                        error!(client = %self.client_id, "Engine to orderbook api channel closed");
                        break;
                    };

                    let instrument = request.instrument.clone();
                    let Some(order_sender) = order_senders.get(&instrument) else {
                        warn!(client = %self.client_id, %instrument, "No orderbook connection for instrument");
                        continue;
                    };

                    match order_sender.try_send(request) {
                        Ok(()) => {}
                        Err(TrySendError::Closed(_)) => {
                            error!(client = %self.client_id, %instrument, "Orderbook order channel closed");
                        }
                        Err(TrySendError::Full(_)) => {
                            warn!(client = %self.client_id, %instrument, "Orderbook order queue full");
                        }
                    }
                }

                result = connections.join_next(), if !connections.is_empty() => {
                    match result {
                        Some(Ok(Ok(()))) => {}
                        Some(Ok(Err(error))) => {
                            error!(client = %self.client_id, %error, "Orderbook connection failed");
                            break;
                        }
                        Some(Err(error)) => {
                            error!(client = %self.client_id, %error, "Orderbook connection task failed");
                            break;
                        }
                        None => break,
                    }
                }
            }
        }

        token.cancel();

        while let Some(result) = connections.join_next().await {
            log_connection_result(self.client_id, result);
        }

        Ok(())
    }
}

fn log_connection_result(client_id: Uuid, result: std::result::Result<Result<()>, JoinError>) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => error!(client = %client_id, %error, "Orderbook connection failed"),
        Err(error) => error!(client = %client_id, %error, "Orderbook connection task failed"),
    }
}
