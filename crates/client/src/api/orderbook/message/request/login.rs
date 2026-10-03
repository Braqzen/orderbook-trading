use serde::Serialize;
use uuid::Uuid;

#[derive(Serialize)]
pub struct LoginRequest {
    client_id: Uuid,
}

impl LoginRequest {
    pub fn new(client_id: Uuid) -> Self {
        Self { client_id }
    }
}
