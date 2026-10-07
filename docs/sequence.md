# Sequence Diagrams

## Client order partially filled

After a client has subscribed to price updates from the `Market Data Provider` and completed the login sequence in an `Orderbook`, an incoming price event causes the client to place an order which is partially traded against existing orders and the unfilled/remainder of the order is added to the orderbook for future trades.

In the case where an order is fully traded there is nothing to add to the book, whereas if no trades occur the full order is added.

```mermaid
sequenceDiagram
    participant Generator as Price Generator
    participant MarketData as Market Data Provider
    participant Client as Trading Client
    participant Orderbook as Instrument Orderbook
    participant OtherClient as Other Trading Client

    Client->>MarketData: Subscribe to the instrument
    Client->>Orderbook: Log in with client ID
    Orderbook-->>Client: Login accepted
    Generator->>MarketData: New simulated price for an instrument
    MarketData->>Client: Price update for the subscribed instrument
    Client->>Client: Decide to place a limit order
    Client->>Client: Choose buy or sell, price, and quantity
    Client->>Client: Reserve the balance needed for the order
    Client->>Orderbook: Place the limit order
    Orderbook->>Orderbook: Match most of it with an existing opposite order
    Orderbook->>Orderbook: Store the remaining quantity for a future match
    Orderbook-->>OtherClient: Trade result for the matched quantity
    Orderbook-->>Client: Trade result showing a partial fill
    Orderbook-->>Client: Order accepted with the remainder left open
    Client->>Client: Apply the fill and track the remaining open quantity
```

## Client successfully cancels an existing order

When a price event comes in a client may choose to cancel an existing order at random to free up some of its inventory for other trades.

```mermaid
sequenceDiagram
    participant MarketData as Market Data Provider
    participant Client as Trading Client
    participant Orderbook as Instrument Orderbook

    Client->>MarketData: Subscribe to the instrument
    Client->>Orderbook: Log in with client ID
    Orderbook-->>Client: Login accepted
    Note over Client,Orderbook: The client has an open order from an earlier placement
    MarketData->>Client: Price update for the subscribed instrument
    Client->>Client: Select one open order at random
    Client->>Orderbook: Cancel the order
    Orderbook->>Orderbook: Remove the remaining quantity
    Orderbook-->>Client: Cancellation confirmed
    Client->>Client: Release the reserved balance and remove the open order
```

## Client requests are rejected before login

An orderbook connection does not accept trading requests until the client has successfully logged in. Placement and cancellation attempts both receive the same not-logged-in response.

```mermaid
sequenceDiagram
    participant Client as Trading Client
    participant Orderbook as Instrument Orderbook

    Client->>Orderbook: Open WebSocket connection without a login sequence
    Client->>Orderbook: Place a limit order
    Orderbook-->>Client: Login rejected: not logged in
    Client->>Orderbook: Cancel an order
    Orderbook-->>Client: Login rejected: not logged in
```
