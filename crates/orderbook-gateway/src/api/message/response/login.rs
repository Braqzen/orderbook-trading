use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct LoginRejection {
    pub reason: LoginRejectionReason,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginRejectionReason {
    ClientAlreadyConnected,
    NotLoggedIn,
}
