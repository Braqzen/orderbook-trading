use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum LoginResponse {
    LoginAccepted,
    LoginRejected(LoginRejection),
}

#[derive(Deserialize)]
pub struct LoginRejection {
    pub reason: LoginRejectionReason,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginRejectionReason {
    ClientAlreadyConnected,
    NotLoggedIn,
}
