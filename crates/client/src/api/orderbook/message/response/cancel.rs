use serde::Deserialize;
use uuid::Uuid;

#[derive(Deserialize, Clone)]
pub struct Cancelled {
    pub order_id: Uuid,
}

#[derive(Deserialize)]
pub struct CancelRejection {
    pub order_id: Uuid,
    pub reason: CancelRejectionReason,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CancelRejectionReason {
    OrderNotFound,
}
