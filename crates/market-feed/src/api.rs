use eyre::Result;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::{
    net::TcpListener,
    select,
    sync::broadcast::{Sender, error::RecvError},
    task::{JoinError, JoinSet},
};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

pub async fn websocket(tx: Sender<f64>, ws: SocketAddr, token: CancellationToken) -> Result<()> {
    let listener = TcpListener::bind(ws).await?;
    let mut connections = JoinSet::new();

    loop {
        select! {
            biased;

            _ = token.cancelled() => break,

            result = connections.join_next(), if !connections.is_empty() => {
                if let Some(result) = result {
                    log_connection_result(result);
                }
            }

            accepted = listener.accept() => {
                let (stream, peer) = match accepted {
                    Ok(value) => value,
                    Err(error) => {
                        error!(%error, "Failed to accept WebSocket connection");
                        continue;
                    }
                };

                let tx = tx.clone();
                let connection_token = token.child_token();

                connections.spawn(async move {
                    let ws = match accept_async(stream).await {
                        Ok(ws) => ws,
                        Err(error) => {
                            error!(%error, "WebSocket handshake failed");
                            return Ok(());
                        }
                    };

                    let (mut sender, mut receiver) = ws.split();
                    let mut rx = tx.subscribe();

                    loop {
                        select! {
                            biased;

                            _ = connection_token.cancelled() => break,

                            message = receiver.next() => {
                                match message {
                                    Some(Ok(Message::Close(_))) | None => break,
                                    Some(Err(error)) => {
                                        error!(%error, "WebSocket connection failed");
                                        break;
                                    }
                                    _ => {}
                                }
                            }

                            price = rx.recv() => {
                                match price {
                                    Ok(price) => {
                                        info!(%peer, price, "Sending price");
                                        if let Err(error) = sender
                                            .send(Message::Text(price.to_string().into()))
                                            .await
                                        {
                                            error!(%error, "Failed to send price");
                                            break;
                                        }
                                    }
                                    Err(RecvError::Lagged(skipped)) => {
                                        warn!(skipped, "WebSocket client lagged");
                                    }
                                    Err(RecvError::Closed) => break,
                                }
                            }
                        }
                    }

                    if let Err(error) = sender.close().await {
                        error!(%error, "Failed to close WebSocket connection");
                    }

                    Ok(())
                });
            }
        }
    }

    token.cancel();

    while let Some(result) = connections.join_next().await {
        log_connection_result(result);
    }

    Ok(())
}

fn log_connection_result(result: std::result::Result<Result<()>, JoinError>) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => error!(%error, "WebSocket connection failed"),
        Err(error) => error!(%error, "WebSocket connection task failed"),
    }
}
