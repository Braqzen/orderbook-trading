//! Session data for one logged-in client.
//!
//! Kept by the session store, separate from the connection that owns the socket, so a client
//! can be found and limited to one active session.

use crate::api::Response;
use tokio::sync::mpsc::{Sender, error::TrySendError};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct ClientRecord {
    /// Sends responses to the connection that writes this client's websocket
    outbound: Sender<Response>,
    /// Cancels this client's connection
    cancel: CancellationToken,
    /// Last execution sequence this client has already applied
    last_exec_seq: u64,
}

impl ClientRecord {
    pub fn new(outbound: Sender<Response>, cancel: CancellationToken, last_exec_seq: u64) -> Self {
        Self {
            outbound,
            cancel,
            last_exec_seq,
        }
    }

    pub fn try_send(&self, message: Response) -> Result<(), TrySendError<Response>> {
        self.outbound.try_send(message)
    }

    pub fn disconnect(&self) {
        self.cancel.cancel();
    }

    pub fn same_outbound(&self, other: &Sender<Response>) -> bool {
        self.outbound.same_channel(other)
    }

    pub fn last_exec_seq(&self) -> u64 {
        self.last_exec_seq
    }

    pub fn outbound(&self) -> Sender<Response> {
        self.outbound.clone()
    }
}
