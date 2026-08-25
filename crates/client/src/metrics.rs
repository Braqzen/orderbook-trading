use crate::trade::{Asset, Instrument, Order, Quantity};
use opentelemetry::{
    KeyValue, global,
    metrics::{Counter, Gauge},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct ClientMetrics {
    client_id: String,
    available: Gauge<f64>,
    reserved: Gauge<f64>,
    open_orders: Gauge<u64>,
    subscriptions: Gauge<u64>,
    orders_submitted: Counter<u64>,
    trades: Counter<u64>,
    trade_quantity: Counter<f64>,
}

impl ClientMetrics {
    pub fn new(client_id: Uuid) -> Self {
        let meter = global::meter("client");

        Self {
            client_id: client_id.to_string(),
            available: meter.f64_gauge("client.inventory.available").build(),
            reserved: meter.f64_gauge("client.inventory.reserved").build(),
            open_orders: meter.u64_gauge("client.orders.open").build(),
            subscriptions: meter.u64_gauge("client.subscriptions").build(),
            orders_submitted: meter.u64_counter("client.orders.submitted_total").build(),
            trades: meter.u64_counter("client.trades_total").build(),
            trade_quantity: meter.f64_counter("client.trade.quantity_total").build(),
        }
    }

    pub fn record_available(&self, asset: &Asset, quantity: Quantity) {
        self.available.record(
            quantity.value(),
            &[
                KeyValue::new("client", self.client_id.clone()),
                KeyValue::new("asset", asset.as_str().to_owned()),
            ],
        );
    }

    pub fn record_reserved(&self, asset: &Asset, quantity: Quantity) {
        self.reserved.record(
            quantity.value(),
            &[
                KeyValue::new("client", self.client_id.clone()),
                KeyValue::new("asset", asset.as_str().to_owned()),
            ],
        );
    }

    pub fn record_open_orders(&self, instrument: &Instrument, count: u64) {
        self.open_orders.record(
            count,
            &[
                KeyValue::new("client", self.client_id.clone()),
                KeyValue::new("instrument", instrument.to_string()),
            ],
        );
    }

    pub fn record_subscription(&self, instrument: &Instrument, subscribed: bool) {
        self.subscriptions.record(
            u64::from(subscribed),
            &[
                KeyValue::new("client", self.client_id.clone()),
                KeyValue::new("instrument", instrument.to_string()),
            ],
        );
    }

    pub fn record_order_submitted(&self, order: &Order) {
        self.orders_submitted.add(
            1,
            &[
                KeyValue::new("client", self.client_id.clone()),
                KeyValue::new("instrument", order.instrument.to_string()),
                KeyValue::new("side", order.side.to_string()),
            ],
        );
    }

    pub fn record_trade(&self, order: &Order, quantity: Quantity) {
        let attributes = [
            KeyValue::new("client", self.client_id.clone()),
            KeyValue::new("instrument", order.instrument.to_string()),
            KeyValue::new("side", order.side.to_string()),
        ];

        self.trades.add(1, &attributes);
        self.trade_quantity.add(quantity.value(), &attributes);
    }
}
