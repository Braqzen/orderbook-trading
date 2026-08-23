use crate::trade::Instrument;
use opentelemetry::{KeyValue, global, metrics::UpDownCounter};
use uuid::Uuid;

#[derive(Clone)]
pub struct OrderbookMetrics {
    instrument: String,
    connected_clients: UpDownCounter<i64>,
    queue_size: UpDownCounter<i64>,
}

impl OrderbookMetrics {
    pub fn new(instrument: &Instrument) -> Self {
        let meter = global::meter("orderbook");

        Self {
            instrument: instrument.to_string(),
            connected_clients: meter
                .i64_up_down_counter("orderbook.clients.connected")
                .build(),
            queue_size: meter.i64_up_down_counter("orderbook.queue.size").build(),
        }
    }

    pub fn client_connected(&self) {
        self.connected_clients
            .add(1, &[KeyValue::new("instrument", self.instrument.clone())]);
    }

    pub fn client_disconnected(&self) {
        self.connected_clients
            .add(-1, &[KeyValue::new("instrument", self.instrument.clone())]);
    }

    pub fn client_order_enqueued(&self, client_id: Uuid) {
        self.queue_size.add(
            1,
            &[
                KeyValue::new("instrument", self.instrument.clone()),
                KeyValue::new("queue", "client"),
                KeyValue::new("client", client_id.to_string()),
            ],
        );
    }

    pub fn client_order_dequeued(&self, client_id: Uuid) {
        self.queue_size.add(
            -1,
            &[
                KeyValue::new("instrument", self.instrument.clone()),
                KeyValue::new("queue", "client"),
                KeyValue::new("client", client_id.to_string()),
            ],
        );
    }

    pub fn client_orders_dropped(&self, client_id: Uuid, count: usize) {
        let count = match i64::try_from(count) {
            Ok(count) => count,
            Err(_) => i64::MAX,
        };

        self.queue_size.add(
            -count,
            &[
                KeyValue::new("instrument", self.instrument.clone()),
                KeyValue::new("queue", "client"),
                KeyValue::new("client", client_id.to_string()),
            ],
        );
    }

    pub fn global_order_enqueued(&self) {
        self.queue_size.add(
            1,
            &[
                KeyValue::new("instrument", self.instrument.clone()),
                KeyValue::new("queue", "global"),
            ],
        );
    }

    pub fn global_order_dequeued(&self) {
        self.queue_size.add(
            -1,
            &[
                KeyValue::new("instrument", self.instrument.clone()),
                KeyValue::new("queue", "global"),
            ],
        );
    }
}
