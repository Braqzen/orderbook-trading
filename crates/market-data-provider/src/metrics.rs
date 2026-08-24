use opentelemetry::{KeyValue, global, metrics::UpDownCounter};

#[derive(Clone)]
pub struct MarketDataProviderMetrics {
    connected_clients: UpDownCounter<i64>,
    subscriptions: UpDownCounter<i64>,
}

impl MarketDataProviderMetrics {
    pub fn new() -> Self {
        let meter = global::meter("market-data-provider");

        Self {
            connected_clients: meter
                .i64_up_down_counter("market_data_provider.clients.connected")
                .build(),
            subscriptions: meter
                .i64_up_down_counter("market_data_provider.subscriptions")
                .build(),
        }
    }

    pub fn client_connected(&self) {
        self.connected_clients.add(1, &[]);
    }

    pub fn client_disconnected(&self) {
        self.connected_clients.add(-1, &[]);
    }

    pub fn instrument_subscribed(&self, instrument: &str) {
        self.subscriptions
            .add(1, &[KeyValue::new("instrument", instrument.to_owned())]);
    }

    pub fn instrument_unsubscribed(&self, instrument: &str) {
        self.subscriptions
            .add(-1, &[KeyValue::new("instrument", instrument.to_owned())]);
    }
}
