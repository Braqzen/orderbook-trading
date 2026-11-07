//! Tracks reconnect delay between orderbook connection attempts for one instrument.

use std::time::Duration;

/// Delay before the first reconnect attempt after a session ends
const INITIAL_DELAY: Duration = Duration::from_secs(1);
/// Upper bound on reconnect delay between attempts
const MAX_DELAY: Duration = Duration::from_secs(60);
/// Stop reconnecting after this many retries at max delay
const MAX_DELAY_RETRIES: u32 = 10;

pub struct ReconnectBackoff {
    /// Wait time before the next connection attempt
    delay: Duration,
    /// Number of reconnects delayed by the maximum
    max_delay_retries: u32,
}

impl ReconnectBackoff {
    pub fn new() -> Self {
        Self {
            delay: INITIAL_DELAY,
            max_delay_retries: 0,
        }
    }

    pub fn reset(&mut self) {
        self.delay = INITIAL_DELAY;
        self.max_delay_retries = 0;
    }

    pub fn retry_delay(&mut self) -> Option<Duration> {
        if self.max_delay_retries >= MAX_DELAY_RETRIES {
            return None;
        }

        let retry_delay = self.delay;

        if self.delay == MAX_DELAY {
            self.max_delay_retries += 1;
        } else {
            self.delay = self.delay.saturating_mul(2).min(MAX_DELAY);
        }

        Some(retry_delay)
    }
}
