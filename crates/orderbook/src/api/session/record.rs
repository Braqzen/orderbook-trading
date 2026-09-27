use crate::api::Response;
use tokio::sync::mpsc::{Sender, error::TrySendError};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct ClientRecord {
    outbound: Sender<Response>,
    cancel: CancellationToken,
}

impl ClientRecord {
    pub fn new(outbound: Sender<Response>, cancel: CancellationToken) -> Self {
        Self { outbound, cancel }
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
}
