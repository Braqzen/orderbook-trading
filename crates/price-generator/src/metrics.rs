use crate::instrument::Instrument;
use opentelemetry::{KeyValue, global, metrics::Gauge};

#[derive(Clone)]
pub struct FeedMetrics {
    /// Track the price per instrument
    price: Gauge<f64>,
}

impl FeedMetrics {
    pub fn new() -> Self {
        let meter = global::meter("price-generator");
        let price = meter.f64_gauge("price_generator.price").build();

        Self { price }
    }

    pub fn record_sent_price(&self, instrument: &Instrument, value: f64) {
        self.price.record(
            value,
            &[KeyValue::new("instrument", instrument.to_string())],
        );
    }
}
