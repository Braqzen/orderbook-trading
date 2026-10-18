use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct LoginRequest {
    client_id: Uuid,
    last_exec_seq: u64,
}

impl LoginRequest {
    pub fn new(client_id: Uuid, last_exec_seq: u64) -> Self {
        Self {
            client_id,
            last_exec_seq,
        }
    }
}
