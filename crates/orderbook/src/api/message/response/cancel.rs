use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct Cancelled {
    pub order_id: Uuid,
}

#[derive(Serialize)]
pub struct CancelRejection {
    pub order_id: Uuid,
    pub reason: CancelRejectionReason,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelRejectionReason {
    OrderNotFound,
}
