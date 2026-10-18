use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct Cancelled {
    pub exec_seq: u64,
    pub order_id: Uuid,
}

#[derive(Serialize)]
pub struct CancelRejection {
    pub exec_seq: u64,
    pub order_id: Uuid,
    pub reason: CancelRejectionReason,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelRejectionReason {
    OrderNotFound,
}
