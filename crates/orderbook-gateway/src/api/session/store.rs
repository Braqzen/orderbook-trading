//! Maps each logged-in client to its live connection so executions can be sent to that socket.
//!
//! Enforces one active session per client ID.

use crate::api::{LoginRejectionReason, LoginRequest, Response, session::record::ClientRecord};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{RwLock, mpsc::Sender};
use tokio_util::sync::CancellationToken;
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

        clients.insert(
            login.client_id,
            ClientRecord::new(outbound, cancel, login.last_exec_seq),
        );

        Ok(())
    }

    pub async fn logout(&self, client_id: Uuid, outbound: &Sender<Response>) {
        let mut clients = self.clients.write().await;
        if clients
            .get(&client_id)
            .is_some_and(|record| record.same_outbound(outbound))
        {
            if let Some(record) = clients.remove(&client_id) {
                record.disconnect();
            }
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
}
