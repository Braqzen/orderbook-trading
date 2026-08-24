use crate::{
    api::{MarketPrice, Response},
    metrics::ClientMetrics,
    trade::{Asset, Instrument, Inventory, Order, OrderType, Quantity},
};
use eyre::Result;
use std::collections::HashMap;
use tokio::{
    select,
    sync::mpsc::{Receiver, Sender},
};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct Engine {
    id: Uuid,
    inventory: Inventory,
    open_orders: HashMap<Uuid, Order>,
    price_receiver_channel: Receiver<MarketPrice>,
    order_sender_channel: Sender<Order>,
    response_receiver_channel: Receiver<Response>,
    metrics: ClientMetrics,
}

impl Engine {
    pub fn new(
        id: Uuid,
        inventory: Inventory,
        price_receiver_channel: Receiver<MarketPrice>,
        order_sender_channel: Sender<Order>,
        response_receiver_channel: Receiver<Response>,
        metrics: ClientMetrics,
    ) -> Self {
        Self {
            id,
            inventory,
            open_orders: HashMap::new(),
            price_receiver_channel,
            order_sender_channel,
            response_receiver_channel,
            metrics,
        }
    }

    pub async fn run(mut self, token: CancellationToken) -> Result<()> {
        for asset in self.inventory.assets() {
            self.record_asset_metrics(&asset);
        }

        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                response = self.response_receiver_channel.recv() => {
                    let Some(response) = response else {
                        error!(client = %self.id, "Orderbook response channel closed");
                        break;
                    };

                    match response {
                        Response::Trade(trade) => {
                            info!(
                                client = %self.id,
                                order = %trade.order_id,
                                side = %trade.side,
                                price = trade.price,
                                size = trade.size,
                                remaining = trade.remaining,
                                "Trade executed"
                            );

                            let Some(order) = self.open_orders.get(&trade.order_id).cloned() else {
                                warn!(client = %self.id, order = %trade.order_id, "Received fill for unknown order");
                                continue;
                            };

                            let fill_price = trade.price as f64 / 100.0;
                            let fill_size = match Quantity::try_from(trade.size as f64) {
                                Ok(fill_size) => fill_size,
                                Err(error) => {
                                    warn!(client = %self.id, %error, order = %trade.order_id, "Invalid fill size");
                                    continue;
                                }
                            };

                            let result = match order.side {
                                OrderType::Buy => self.inventory.apply_buy(&order, fill_size, fill_price),
                                OrderType::Sell => self.inventory.apply_sell(&order, fill_size, fill_price),
                            };

                            match result {
                                Ok(()) => {
                                    self.metrics.record_trade(&order, fill_size);
                                    self.record_asset_metrics(order.instrument.base());
                                    self.record_asset_metrics(order.instrument.quote());

                                    if trade.remaining == 0 {
                                        self.open_orders.remove(&trade.order_id);
                                        self.record_open_orders_metrics(&order.instrument);
                                    }
                                }
                                Err(error) => {
                                    warn!(
                                        client = %self.id,
                                        %error,
                                        order = %trade.order_id,
                                        "Failed to apply fill to inventory"
                                    );
                                }
                            }
                        }
                        Response::Rejection(rejection) => {
                            if !self.open_orders.contains_key(&rejection.order_id) {
                                warn!(client = %self.id, order = %rejection.order_id, "Received rejection for unknown order");
                                continue;
                            }

                            let price = rejection.price as f64 / 100.0;
                            let (asset, amount) = match rejection.side {
                                OrderType::Buy => (
                                    rejection.instrument.quote(),
                                    price * rejection.size as f64,
                                ),
                                OrderType::Sell => (
                                    rejection.instrument.base(),
                                    rejection.size as f64,
                                ),
                            };
                            let amount = match Quantity::try_from(amount) {
                                Ok(amount) => amount,
                                Err(error) => {
                                    warn!(
                                        client = %self.id,
                                        %error,
                                        order = %rejection.order_id,
                                        "Invalid rejected order reserve amount"
                                    );
                                    continue;
                                }
                            };

                            if let Err(error) = self.inventory.release(asset, amount) {
                                warn!(
                                    client = %self.id,
                                    %error,
                                    order = %rejection.order_id,
                                    "Failed to release rejected order inventory"
                                );
                                continue;
                            }

                            self.open_orders.remove(&rejection.order_id);
                            self.record_asset_metrics(asset);
                            self.record_open_orders_metrics(&rejection.instrument);

                            warn!(
                                client = %self.id,
                                order = %rejection.order_id,
                                instrument = %rejection.instrument,
                                ?rejection.reason,
                                "Order rejection applied"
                            );
                        }
                    }
                }

                price = self.price_receiver_channel.recv() => {
                    let Some(MarketPrice { instrument, value }) = price else {
                        error!(client = %self.id, "Market data provider channel closed");
                        break;
                    };

                    let base = self.inventory.available(instrument.base());
                    let quote = self.inventory.available(instrument.quote());

                    // TODO: strategies will be implemented later, rn we only care the instrument exists
                    //       this causes errors because we do not check amounts but nothing should break
                    let (_base, _quote) = match (base, quote) {
                        (Some(base), Some(quote)) => {
                            (base, quote)
                        },
                        (_, _) => continue
                    };

                    let side = if rand::random_bool(0.5) {
                        OrderType::Buy
                    } else {
                        OrderType::Sell
                    };
                    let size = rand::random_range(1..5);
                    let order_id = Uuid::new_v4();

                    let order = Order::new(instrument.clone(), value, size, side, self.id, order_id);

                    let (asset, amount) = match side {
                        OrderType::Buy => (instrument.quote(), value * size as f64),
                        OrderType::Sell => (instrument.base(), size as f64),
                    };

                    let amount = match Quantity::try_from(amount) {
                        Ok(amount) => amount,
                        Err(error) => {
                            warn!(client = %self.id, %error, %instrument, %side, size, value, "Invalid reserve amount");
                            continue;
                        }
                    };

                    if let Err(error) = self.inventory.reserve(asset, amount) {
                        warn!(client = %self.id, %error, %instrument, %side, size, value, "Failed to reserve inventory");
                        continue;
                    }

                    self.record_asset_metrics(asset);

                    info!(%instrument, value, size, %side, client=%self.id, order=%order_id, "Created order");

                    if self.order_sender_channel.send(order.clone()).await.is_err() {
                        if let Err(error) = self.inventory.release(asset, amount) {
                            error!(client = %self.id, %error, %instrument, %side, size, value, "Failed to release unsent order inventory");
                        } else {
                            self.record_asset_metrics(asset);
                        }
                        error!(client = %self.id, "Order channel closed");
                        break;
                    }

                    self.metrics.record_order_submitted(&order);
                    self.open_orders.insert(order.order_id, order);
                    self.record_open_orders_metrics(&instrument);
                }
            }
        }

        Ok(())
    }

    fn record_asset_metrics(&self, asset: &Asset) {
        if let Some(available) = self.inventory.available(asset) {
            self.metrics.record_available(asset, available);
        }

        if let Some(reserved) = self.inventory.reserved(asset) {
            self.metrics.record_reserved(asset, reserved);
        }
    }

    fn record_open_orders_metrics(&self, instrument: &Instrument) {
        let count = self
            .open_orders
            .values()
            .filter(|order| &order.instrument == instrument)
            .count() as u64;

        self.metrics.record_open_orders(instrument, count);
    }
}
