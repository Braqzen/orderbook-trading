use crate::{
    api::{MarketPrice, Rejection, Response, Trade},
    metrics::ClientMetrics,
    trade::{Asset, Instrument, Inventory, Order, OrderType, Price, Quantity, TradeAction, Trader},
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
    trader: Trader,
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
        trader: Trader,
        price_receiver_channel: Receiver<MarketPrice>,
        order_sender_channel: Sender<Order>,
        response_receiver_channel: Receiver<Response>,
        metrics: ClientMetrics,
    ) -> Self {
        Self {
            id,
            inventory,
            trader,
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
                        Response::Trade(trade) => self.handle_trade(trade),
                        Response::Rejection(rejection) => self.handle_rejection(rejection),
                    }
                }

                price = self.price_receiver_channel.recv() => {
                    let Some(price) = price else {
                        error!(client = %self.id, "Market data provider channel closed");
                        break;
                    };

                    if !self.handle_price(price).await {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_trade(&mut self, trade: Trade) {
        let fill_size = Quantity::from(trade.size);
        let remaining = Quantity::from(trade.remaining);
        let fill_price = match Price::try_from(trade.price) {
            Ok(fill_price) => fill_price,
            Err(error) => {
                warn!(client = %self.id, %error, order = %trade.order_id, "Invalid fill price");
                return;
            }
        };

        info!(
            client = %self.id,
            order = %trade.order_id,
            side = %trade.side,
            price = %fill_price,
            size = %fill_size,
            remaining = %remaining,
            "Trade executed"
        );

        let Some(order) = self.open_orders.get(&trade.order_id) else {
            warn!(client = %self.id, order = %trade.order_id, "Received fill for unknown order");
            return;
        };

        let result = match order.side {
            OrderType::Buy => self.inventory.apply_buy(order, fill_size, fill_price),
            OrderType::Sell => self.inventory.apply_sell(order, fill_size, fill_price),
        };

        if let Err(error) = result {
            warn!(
                client = %self.id,
                %error,
                order = %trade.order_id,
                "Failed to apply fill to inventory"
            );
            return;
        }

        self.metrics.record_trade(order, fill_size);
        self.record_asset_metrics(order.instrument.base());
        self.record_asset_metrics(order.instrument.quote());

        if trade.remaining == 0 {
            let Some(order) = self.open_orders.remove(&trade.order_id) else {
                warn!(client = %self.id, order = %trade.order_id, "Filled order no longer exists");
                return;
            };
            self.record_open_orders_metrics(&order.instrument);
        } else if let Some(order) = self.open_orders.get_mut(&trade.order_id) {
            order.size = remaining;
        }
    }

    fn handle_rejection(&mut self, rejection: Rejection) {
        let rejection_price = match Price::try_from(rejection.price) {
            Ok(rejection_price) => rejection_price,
            Err(error) => {
                warn!(client = %self.id, %error, order = %rejection.order_id, "Invalid rejection price");
                return;
            }
        };

        let Some(order) = self.open_orders.get(&rejection.order_id).cloned() else {
            warn!(client = %self.id, order = %rejection.order_id, "Received rejection for unknown order");
            return;
        };

        let (asset, amount) = match order.side {
            OrderType::Buy => {
                let amount = match order.size * order.price {
                    Ok(amount) => amount,
                    Err(error) => {
                        warn!(
                            client = %self.id,
                            %error,
                            order = %rejection.order_id,
                            "Invalid rejected order reserve amount"
                        );
                        return;
                    }
                };
                (order.instrument.quote(), amount)
            }
            OrderType::Sell => (order.instrument.base(), order.size),
        };

        if amount <= Quantity::ZERO {
            warn!(
                client = %self.id,
                error = "rejected order reserve amount must be positive",
                order = %rejection.order_id,
                "Invalid rejected order reserve amount"
            );
            return;
        }

        if let Err(error) = self.inventory.release(asset, amount) {
            warn!(
                client = %self.id,
                %error,
                order = %rejection.order_id,
                "Failed to release rejected order inventory"
            );
            return;
        }

        self.open_orders.remove(&rejection.order_id);
        self.record_asset_metrics(asset);
        self.record_open_orders_metrics(&rejection.instrument);

        warn!(
            client = %self.id,
            order = %rejection.order_id,
            instrument = %rejection.instrument,
            price = %rejection_price,
            size = %Quantity::from(rejection.size),
            side = %rejection.side,
            ?rejection.reason,
            "Order rejection applied"
        );
    }

    async fn handle_price(&mut self, price: MarketPrice) -> bool {
        let TradeAction::Place {
            instrument,
            price: value,
            size,
            side,
        } = self.trader.evaluate(price, &self.inventory)
        else {
            return true;
        };

        let order_id = Uuid::new_v4();
        let order = Order::new(instrument.clone(), value, size, side, self.id, order_id);

        let (asset, amount) = match side {
            OrderType::Buy => {
                let amount = match size * value {
                    Ok(amount) => amount,
                    Err(error) => {
                        warn!(client = %self.id, %error, %instrument, %side, %size, price = %value, "Invalid reserve amount");
                        return true;
                    }
                };
                (instrument.quote(), amount)
            }
            OrderType::Sell => (instrument.base(), size),
        };

        if let Err(error) = self.inventory.reserve(asset, amount) {
            warn!(client = %self.id, %error, %instrument, %side, %size, price = %value, "Failed to reserve inventory");
            return true;
        }

        self.record_asset_metrics(asset);
        info!(%instrument, price = %value, %size, %side, client=%self.id, order=%order_id, "Created order");

        if self.order_sender_channel.send(order.clone()).await.is_err() {
            if let Err(error) = self.inventory.release(asset, amount) {
                error!(client = %self.id, %error, %instrument, %side, %size, price = %value, "Failed to release unsent order inventory");
            } else {
                self.record_asset_metrics(asset);
            }
            error!(client = %self.id, "Order channel closed");
            return false;
        }

        self.metrics.record_order_submitted(&order);
        self.open_orders.insert(order.order_id, order);
        self.record_open_orders_metrics(&instrument);
        true
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
