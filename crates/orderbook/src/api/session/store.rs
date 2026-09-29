//! Maps each logged-in client to its live connection so the engine can send responses.
//! Enforces one active session per client ID.

use crate::{
    api::{LoginRejectionReason, Response, order::LoginRequest, session::record::ClientRecord},
    trade::Instrument,
};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{
    RwLock,
    mpsc::{Sender, error::TrySendError},
};
use tokio_util::sync::CancellationToken;
use tracing::warn;
use uuid::Uuid;

#[derive(Clone)]
pub struct SessionStore {
    /// Associates logged in clients to their information
    clients: Arc<RwLock<HashMap<Uuid, ClientRecord>>>,
}

impl SessionStore {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn login(
        &self,
        login: &LoginRequest,
        outbound: Sender<Response>,
        cancel: CancellationToken,
    ) -> Result<(), LoginRejectionReason> {
        let mut clients = self.clients.write().await;
        if clients.contains_key(&login.client_id) {
            return Err(LoginRejectionReason::ClientAlreadyConnected);
        }

        clients.insert(login.client_id, ClientRecord::new(outbound, cancel));

        Ok(())
    }

    pub async fn logout(&self, client_id: Uuid, outbound: &Sender<Response>) {
        let mut clients = self.clients.write().await;
        if clients
            .get(&client_id)
            .is_some_and(|record| record.same_outbound(outbound))
        {
            clients.remove(&client_id);
        }
    }

    pub async fn verify_auth(&self, client_id: Uuid) -> Result<(), LoginRejectionReason> {
        let clients = self.clients.read().await;
        if clients.get(&client_id).is_none() {
            return Err(LoginRejectionReason::NotLoggedIn);
        }

        Ok(())
    }

    pub async fn client_record(&self, client_id: Uuid) -> Option<ClientRecord> {
        self.clients.read().await.get(&client_id).cloned()
    }

    // TODO: a session store is a data structure and not an actor. Sending should not be here
    //       sending responses ought to be another task or send to a queue, worker sends to user
    //       but if worker sends how does worker connect to user if orderbook is connected?
    pub async fn send_response(
        &self,
        instrument: &Instrument,
        client_id: Uuid,
        response: Response,
    ) {
        let Some(client) = self.client_record(client_id).await else {
            warn!(%instrument, %client_id, "Client is not connected");
            return;
        };

        match client.try_send(response) {
            Ok(()) => {}
            Err(TrySendError::Closed(_)) => {
                warn!(%instrument, %client_id, "Client is not connected");
            }
            Err(TrySendError::Full(_)) => {
                warn!(%instrument, %client_id, "Client outbound queue full");
                client.disconnect();
                self.clients.write().await.remove(&client_id);
            }
        }
    }
}
