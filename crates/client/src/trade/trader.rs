use crate::{
    api::{Cancelled, MarketPrice, OrderRejection, Trade},
    metrics::ClientMetrics,
    trade::{
        Asset, Instrument, Inventory, ORDER_SIZE_ATOM_STEP, Order, OrderType, Price, Quantity,
        TradeAction, TradeLimit,
    },
};
use std::collections::HashMap;
use tracing::{error, info, warn};
use uuid::Uuid;

// TODO: expand to include strategies, perhaps general or per instrument
pub struct Trader {
    /// Track the assets and balances
    inventory: Inventory,
    /// Trading limits associated with each asset
    limits: HashMap<Asset, TradeLimit>,
    /// Which orders have been sent to the orderbook and are still live
    open_orders: HashMap<Uuid, Order>,
    /// Tracks metrics
    metrics: ClientMetrics,
}

impl Trader {
    pub fn new(
        limits: HashMap<Asset, TradeLimit>,
        inventory: Inventory,
        metrics: ClientMetrics,
    ) -> Self {
        Self {
            inventory,
            limits,
            open_orders: HashMap::new(),
            metrics,
        }
    }

    pub fn process_event(&self, price: MarketPrice) -> TradeAction {
        let roll = rand::random::<f64>();

        let action = if roll < 0.5 {
            TradeAction::Skip
        } else if roll < 0.9 {
            self.place_action(price)
        } else {
            self.cancel_action()
        };

        let action_name = match action {
            TradeAction::Skip => "skip",
            TradeAction::Place { .. } => "place",
            TradeAction::Cancel { .. } => "cancel",
        };

        self.metrics.record_trader_action(action_name);

        action
    }

    pub fn prepare_place(
        &mut self,
        client_id: Uuid,
        instrument: Instrument,
        price: Price,
        size: Quantity,
        side: OrderType,
    ) -> Option<Order> {
        let order_id = Uuid::new_v4();
        let order = Order::new(instrument.clone(), price, size, side, client_id, order_id);

        let (asset, amount) = match side {
            OrderType::Buy => {
                let amount = match size * price {
                    Ok(amount) => amount,
                    Err(error) => {
                        warn!(client = %client_id, %error, %instrument, %side, %size, price = %price, "Invalid reserve amount");
                        return None;
                    }
                };
                (instrument.quote().clone(), amount)
            }
            OrderType::Sell => (instrument.base().clone(), size),
        };

        if let Err(error) = self.inventory.reserve(&asset, amount) {
            warn!(client = %client_id, %error, %instrument, %side, %size, price = %price, "Failed to reserve inventory");
            return None;
        }

        self.record_asset(&asset);
        info!(%instrument, price = %price, %size, %side, client=%client_id, order=%order_id, "Created order");

        Some(order)
    }

    pub fn confirm_place(&mut self, order: Order) {
        self.metrics.record_order_submitted(&order);
        let instrument = order.instrument.clone();
        self.open_orders.insert(order.order_id, order);
        self.record_open_orders(&instrument);
    }

    pub fn rollback_place(&mut self, order: &Order) {
        let Some((asset, amount)) = self.reserve(order) else {
            return;
        };

        if let Err(error) = self.inventory.release(&asset, amount) {
            error!(
                client = %order.client_id,
                %error,
                instrument = %order.instrument,
                side = %order.side,
                size = %order.size,
                price = %order.price,
                "Failed to release unsent order inventory"
            );
            return;
        }

        self.record_asset(&asset);
    }

    pub fn order_for_cancel(&self, order_id: Uuid) -> Option<Order> {
        self.open_orders.get(&order_id).cloned()
    }

    pub fn apply_trade(&mut self, client_id: Uuid, trade: Trade) {
        let fill_size = Quantity::from(trade.size);
        let remaining = Quantity::from(trade.remaining);
        let fill_price = match Price::try_from(trade.price) {
            Ok(fill_price) => fill_price,
            Err(error) => {
                warn!(client = %client_id, %error, order = %trade.order_id, "Invalid fill price");
                return;
            }
        };

        info!(
            client = %client_id,
            order = %trade.order_id,
            side = %trade.side,
            price = %fill_price,
            size = %fill_size,
            remaining = %remaining,
            "Trade executed"
        );

        let Some(order) = self.open_orders.get(&trade.order_id).cloned() else {
            warn!(client = %client_id, order = %trade.order_id, "Received fill for unknown order");
            return;
        };

        let result = match order.side {
            OrderType::Buy => self.inventory.apply_buy(&order, fill_size, fill_price),
            OrderType::Sell => self.inventory.apply_sell(&order, fill_size, fill_price),
        };

        if let Err(error) = result {
            warn!(
                client = %client_id,
                %error,
                order = %trade.order_id,
                "Failed to apply fill to inventory"
            );
            return;
        }

        self.metrics.record_trade(&order, fill_size);
        self.record_asset(order.instrument.base());
        self.record_asset(order.instrument.quote());

        if trade.remaining == 0 {
            let Some(order) = self.open_orders.remove(&trade.order_id) else {
                warn!(client = %client_id, order = %trade.order_id, "Filled order no longer exists");
                return;
            };
            self.record_open_orders(&order.instrument);
        } else if let Some(order) = self.open_orders.get_mut(&trade.order_id) {
            order.size = remaining;
        }
    }

    pub fn apply_order_rejection(&mut self, client_id: Uuid, rejection: OrderRejection) {
        let rejection_price = match Price::try_from(rejection.price) {
            Ok(rejection_price) => rejection_price,
            Err(error) => {
                warn!(client = %client_id, %error, order = %rejection.order_id, "Invalid rejection price");
                return;
            }
        };

        let Some(order) = self.open_orders.get(&rejection.order_id).cloned() else {
            warn!(client = %client_id, order = %rejection.order_id, "Received rejection for unknown order");
            return;
        };

        if !self.release_reserve(client_id, &order) {
            return;
        }

        self.open_orders.remove(&rejection.order_id);
        self.record_asset(order.instrument.quote());
        self.record_asset(order.instrument.base());
        self.record_open_orders(&rejection.instrument);

        warn!(
            client = %client_id,
            order = %rejection.order_id,
            instrument = %rejection.instrument,
            price = %rejection_price,
            size = %Quantity::from(rejection.size),
            side = %rejection.side,
            ?rejection.reason,
            "Order rejection applied"
        );
    }

    pub fn apply_cancelled(&mut self, client_id: Uuid, cancelled: Cancelled) {
        let Some(order) = self.open_orders.get(&cancelled.order_id).cloned() else {
            warn!(client = %client_id, order = %cancelled.order_id, "Received cancel for unknown order");
            return;
        };

        if !self.release_reserve(client_id, &order) {
            return;
        }

        self.open_orders.remove(&cancelled.order_id);
        self.record_asset(order.instrument.quote());
        self.record_asset(order.instrument.base());
        self.record_open_orders(&order.instrument);

        info!(
            client = %client_id,
            order = %cancelled.order_id,
            instrument = %order.instrument,
            price = %order.price,
            size = %order.size,
            side = %order.side,
            "Order cancel applied"
        );
    }

    fn place_action(&self, price: MarketPrice) -> TradeAction {
        if self.inventory.available(price.instrument.base()).is_none()
            || self.inventory.available(price.instrument.quote()).is_none()
        {
            return TradeAction::Skip;
        }

        let Some(limit) = self.limits.get(price.instrument.base()) else {
            return TradeAction::Skip;
        };

        let side = if rand::random_bool(0.5) {
            OrderType::Buy
        } else {
            OrderType::Sell
        };

        let size =
            rand::random_range(limit.minimum_size.to_decimals()..=limit.maximum_size.to_decimals())
                * ORDER_SIZE_ATOM_STEP;

        TradeAction::Place {
            instrument: price.instrument,
            price: price.value,
            size: Quantity::from(size),
            side,
        }
    }

    fn cancel_action(&self) -> TradeAction {
        if self.open_orders.is_empty() {
            return TradeAction::Skip;
        }

        let index = rand::random_range(0..self.open_orders.len());
        let Some(order_id) = self.open_orders.keys().nth(index).copied() else {
            return TradeAction::Skip;
        };

        TradeAction::Cancel { order_id }
    }

    fn reserve(&self, order: &Order) -> Option<(Asset, Quantity)> {
        match order.side {
            OrderType::Buy => match order.size * order.price {
                Ok(amount) => Some((order.instrument.quote().clone(), amount)),
                Err(error) => {
                    warn!(
                        client = %order.client_id,
                        %error,
                        order = %order.order_id,
                        "Invalid order reserve amount"
                    );
                    None
                }
            },
            OrderType::Sell => Some((order.instrument.base().clone(), order.size)),
        }
    }

    fn release_reserve(&mut self, client_id: Uuid, order: &Order) -> bool {
        let Some((asset, amount)) = self.reserve(order) else {
            return false;
        };

        if amount <= Quantity::ZERO {
            warn!(
                client = %client_id,
                error = "order reserve amount must be positive",
                order = %order.order_id,
                "Invalid order reserve amount"
            );
            return false;
        }

        if let Err(error) = self.inventory.release(&asset, amount) {
            warn!(
                client = %client_id,
                %error,
                order = %order.order_id,
                "Failed to release order inventory"
            );
            return false;
        }

        self.record_asset(&asset);
        true
    }

    pub fn record_inventory(&self) {
        for asset in self.inventory.assets() {
            self.record_asset(&asset);
        }
    }

    fn record_asset(&self, asset: &Asset) {
        if let Some(available) = self.inventory.available(asset) {
            self.metrics.record_available(asset, available);
        }

        if let Some(reserved) = self.inventory.reserved(asset) {
            self.metrics.record_reserved(asset, reserved);
        }
    }

    fn record_open_orders(&self, instrument: &Instrument) {
        let count = self
            .open_orders
            .values()
            .filter(|order| &order.instrument == instrument)
            .count() as u64;

        self.metrics.record_open_orders(instrument, count);
    }
}
