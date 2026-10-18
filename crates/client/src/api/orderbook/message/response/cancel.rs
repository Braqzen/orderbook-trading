use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Clone)]
pub struct Cancelled {
    pub exec_seq: u64,
    pub order_id: Uuid,
}

#[derive(Deserialize)]
pub struct CancelRejection {
    pub exec_seq: u64,
    pub order_id: Uuid,
    pub reason: CancelRejectionReason,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelRejectionReason {
    OrderNotFound,
}
