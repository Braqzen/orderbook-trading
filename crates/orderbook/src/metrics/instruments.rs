use crate::trade::{Instrument, OrderBook, OrderType, Price, Quantity};
use opentelemetry::{
    KeyValue, global,
    metrics::{Counter, Gauge, UpDownCounter},
};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone)]
pub struct OrderbookMetrics {
    instrument: String,
    connected_clients: UpDownCounter<i64>,
    queue_size: UpDownCounter<i64>,
    level_orders: Gauge<u64>,
    level_quantity: Gauge<f64>,
    best_price: Gauge<f64>,
    worst_price: Gauge<f64>,
    trades: Counter<u64>,
    trade_size: Counter<f64>,
    // Since whole book scanned, we must reset removed levels to 0 or metrics retain last
    // value which inflated panels
    recorded_levels: HashSet<(OrderType, Price)>,
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
            level_orders: meter.u64_gauge("orderbook.level.orders").build(),
            level_quantity: meter.f64_gauge("orderbook.level.quantity").build(),
            best_price: meter.f64_gauge("orderbook.price.best").build(),
            worst_price: meter.f64_gauge("orderbook.price.worst").build(),
            trades: meter.u64_counter("orderbook.trades_total").build(),
            trade_size: meter.f64_counter("orderbook.trade.size_total").build(),
            recorded_levels: HashSet::new(),
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

    pub fn record_orderbook(
        &mut self,
        book: &OrderBook,
        trades: u64,
        trade_size: Quantity,
    ) -> Result<(), String> {
        let current_levels: HashSet<(OrderType, Price)> = book
            .buy_levels()
            .map(|(price, _)| (OrderType::Buy, *price))
            .chain(
                book.sell_levels()
                    .map(|(price, _)| (OrderType::Sell, *price)),
            )
            .collect();

        for (side, price) in self.recorded_levels.difference(&current_levels) {
            let attributes = [
                KeyValue::new("instrument", self.instrument.clone()),
                KeyValue::new("side", side.to_string()),
                KeyValue::new("price", price.to_string()),
            ];
            self.level_orders.record(0, &attributes);
            self.level_quantity.record(0.0, &attributes);
        }

        for (price, level) in book.buy_levels() {
            let attributes = [
                KeyValue::new("instrument", self.instrument.clone()),
                KeyValue::new("side", OrderType::Buy.to_string()),
                KeyValue::new("price", price.to_string()),
            ];
            self.level_orders
                .record(level.orders().count() as u64, &attributes);
            self.level_quantity
                .record(level.quantity()?.as_units(), &attributes);
        }

        for (price, level) in book.sell_levels() {
            let attributes = [
                KeyValue::new("instrument", self.instrument.clone()),
                KeyValue::new("side", OrderType::Sell.to_string()),
                KeyValue::new("price", price.to_string()),
            ];
            self.level_orders
                .record(level.orders().count() as u64, &attributes);
            self.level_quantity
                .record(level.quantity()?.as_units(), &attributes);
        }

        let buy_attributes = [
            KeyValue::new("instrument", self.instrument.clone()),
            KeyValue::new("side", OrderType::Buy.to_string()),
        ];
        self.best_price.record(
            book.buy_levels()
                .next_back()
                .map_or(0.0, |(price, _)| price.as_units()),
            &buy_attributes,
        );
        self.worst_price.record(
            book.buy_levels()
                .next()
                .map_or(0.0, |(price, _)| price.as_units()),
            &buy_attributes,
        );

        let sell_attributes = [
            KeyValue::new("instrument", self.instrument.clone()),
            KeyValue::new("side", OrderType::Sell.to_string()),
        ];
        self.best_price.record(
            book.sell_levels()
                .next()
                .map_or(0.0, |(price, _)| price.as_units()),
            &sell_attributes,
        );
        self.worst_price.record(
            book.sell_levels()
                .next_back()
                .map_or(0.0, |(price, _)| price.as_units()),
            &sell_attributes,
        );

        let attributes = &[KeyValue::new("instrument", self.instrument.clone())];
        self.trades.add(trades, attributes);
        self.trade_size.add(trade_size.as_units(), attributes);
        self.recorded_levels = current_levels;

        Ok(())
    }
}
