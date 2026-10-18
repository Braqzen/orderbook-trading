//! gRPC client that submits place and cancel requests to the orderbook.
//!
//! Converts the internal request into the protobuf wire type before sending.

use crate::{
    proto::{self, Side, SubmitRequest, orderbook_client::OrderbookClient, submit_request::Action},
    trade::{OrderType, Request},
};
use eyre::{Result, eyre};
use tonic::transport::Channel;

#[derive(Clone)]
pub struct Submitter {
    client: OrderbookClient<Channel>,
}

impl Submitter {
    pub async fn connect(url: &str) -> Result<Self> {
        let client = OrderbookClient::connect(url.to_owned()).await?;

        Ok(Self { client })
    }

    pub async fn submit(&self, request: Request) -> Result<()> {
        self.client
            .clone()
            .submit(create_request(request))
            .await
            .map(|_| ())
            .map_err(|error| eyre!(error))
    }
}

fn create_request(request: Request) -> SubmitRequest {
    match request {
        Request::Place {
            instrument,
            price,
            order,
        } => SubmitRequest {
            client_id: order.client_id.to_string(),
            action: Some(Action::Place(proto::Place {
                instrument: instrument.to_string(),
                price: price.cents(),
                size: order.size.atoms(),
                side: side(order.side),
                order_id: order.order_id.to_string(),
            })),
        },
        Request::Cancel {
            client_id,
            order_id,
            price,
            side: order_side,
        } => SubmitRequest {
            client_id: client_id.to_string(),
            action: Some(Action::Cancel(proto::Cancel {
                order_id: order_id.to_string(),
                price: price.cents(),
                side: side(order_side),
            })),
        },
    }
}

fn side(side: OrderType) -> i32 {
    match side {
        OrderType::Buy => Side::Buy as i32,
        OrderType::Sell => Side::Sell as i32,
    }
}
