use crate::{
    api::{MarketPrice, Response},
    trade::{Inventory, Order, OrderType, Quantity},
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
}

impl Engine {
    pub fn new(
        inventory: Inventory,
        price_receiver_channel: Receiver<MarketPrice>,
        order_sender_channel: Sender<Order>,
        response_receiver_channel: Receiver<Response>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            inventory,
            open_orders: HashMap::new(),
            price_receiver_channel,
            order_sender_channel,
            response_receiver_channel,
        }
    }

    pub async fn run(mut self, token: CancellationToken) -> Result<()> {
        loop {
            select! {
                biased;

                _ = token.cancelled() => break,

                response = self.response_receiver_channel.recv() => {
                    let Some(response) = response else {
                        error!("Orderbook response channel closed");
                        break;
                    };

                    match response {
                        Response::Trade(trade) => {
                            info!(
                                order = %trade.order_id,
                                side = %trade.side,
                                price = trade.price,
                                size = trade.size,
                                remaining = trade.remaining,
                                "Trade executed"
                            );

                            let Some(order) = self.open_orders.get(&trade.order_id).cloned() else {
                                warn!(order = %trade.order_id, "Received fill for unknown order");
                                continue;
                            };

                            let fill_price = trade.price as f64 / 100.0;
                            let fill_size = match Quantity::try_from(trade.size as f64) {
                                Ok(fill_size) => fill_size,
                                Err(error) => {
                                    warn!(%error, order = %trade.order_id, "Invalid fill size");
                                    continue;
                                }
                            };

                            let result = match order.side {
                                OrderType::Buy => self.inventory.apply_buy(
                                    order.instrument.base(),
                                    order.instrument.quote(),
                                    fill_size,
                                    fill_price,
                                    order.price,
                                ),
                                OrderType::Sell => self.inventory.apply_sell(
                                    order.instrument.base(),
                                    order.instrument.quote(),
                                    fill_size,
                                    fill_price,
                                ),
                            };

                            match result {
                                Ok(()) => {
                                    if trade.remaining == 0 {
                                        self.open_orders.remove(&trade.order_id);
                                    }
                                }
                                Err(error) => {
                                    warn!(
                                        %error,
                                        order = %trade.order_id,
                                        "Failed to apply fill to inventory"
                                    );
                                }
                            }
                        }
                        Response::Rejection(rejection) => {
                            if !self.open_orders.contains_key(&rejection.order_id) {
                                warn!(order = %rejection.order_id, "Received rejection for unknown order");
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
                                        %error,
                                        order = %rejection.order_id,
                                        "Invalid rejected order reserve amount"
                                    );
                                    continue;
                                }
                            };

                            if let Err(error) = self.inventory.release(asset, amount) {
                                warn!(
                                    %error,
                                    order = %rejection.order_id,
                                    "Failed to release rejected order inventory"
                                );
                                continue;
                            }

                            self.open_orders.remove(&rejection.order_id);

                            warn!(
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
                        error!("Market feed channel closed");
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
                            warn!(%error, %instrument, %side, size, value, "Invalid reserve amount");
                            continue;
                        }
                    };

                    if let Err(error) = self.inventory.reserve(asset, amount) {
                        warn!(%error, %instrument, %side, size, value, "Failed to reserve inventory");
                        continue;
                    }

                    info!(%instrument, value, size, %side, client=%self.id, order=%order_id, "Created order");

                    if self.order_sender_channel.send(order.clone()).await.is_err() {
                        if let Err(error) = self.inventory.release(asset, amount) {
                            error!(%error, %instrument, %side, size, value, "Failed to release unsent order inventory");
                        }
                        error!("Order channel closed");
                        break;
                    }

                    self.open_orders.insert(order.order_id, order);
                }
            }
        }

        Ok(())
    }
}
