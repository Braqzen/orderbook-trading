use serde::Serialize;

#[derive(Serialize)]
pub struct LoginRejection {
    pub reason: LoginRejectionReason,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginRejectionReason {
    ClientAlreadyConnected,
    NotLoggedIn,
}
