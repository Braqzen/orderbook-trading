use eyre::{Result, ensure};
use std::fmt::{self, Display, Formatter};

#[derive(Debug, Clone)]
pub struct WsUrl(String);

impl WsUrl {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for WsUrl {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl TryFrom<&str> for WsUrl {
    type Error = eyre::Report;

    fn try_from(value: &str) -> Result<Self> {
        ensure!(
            value.starts_with("ws://") || value.starts_with("wss://"),
            "Invalid websocket URL: {value}"
        );

        Ok(Self(value.to_owned()))
    }
}
