# Sequence Diagrams

## Client order partially filled

After a client has subscribed to price updates from the `Market Data Gateway` and completed the login sequence on an `Orderbook Gateway`, an incoming price event causes the client to place an order which is partially traded against existing orders and the unfilled/remainder of the order is added to the orderbook for future trades.

In the case where an order is fully traded there is nothing to add to the book, whereas if no trades occur the full order is added.

```mermaid
sequenceDiagram
    participant Generator as Price Generator
    participant MarketData as Price Aggregator
    participant Cache as Price Cache
    participant Gateway as Market Data Gateway
    participant Client as Trading Client
    participant OrderGateway as Orderbook Gateway
    participant Orderbook as Instrument Orderbook
    participant Log as Execution Log
    participant OtherClient as Other Trading Client

    Client->>Gateway: Subscribe to the instrument
    Client->>OrderGateway: Log in with client ID and last execution sequence
    OrderGateway-->>Client: Login accepted
    Generator->>MarketData: New simulated price for an instrument
    MarketData->>Cache: Store the latest price
    MarketData->>Cache: Publish the instrument
    Cache-->>Gateway: Publish received
    Gateway->>Cache: Read the latest price
    Gateway->>Client: Price update for the subscribed instrument
    Client->>Client: Decide to place a limit order
    Client->>Client: Choose buy or sell, price, and quantity
    Client->>Client: Reserve the balance needed for the order
    Client->>OrderGateway: Place the limit order
    OrderGateway->>Orderbook: Submit the limit order
    Orderbook->>Orderbook: Match most of it with an existing opposite order
    Orderbook->>Orderbook: Store the remaining quantity for a future match
    Orderbook->>Log: Publish the trade and the accepted remainder
    Log-->>OrderGateway: Trade and accepted remainder for each client
    OrderGateway-->>OtherClient: Trade result for the matched quantity
    OrderGateway-->>Client: Trade result showing a partial fill
    OrderGateway-->>Client: Order accepted with the remainder left open
    Client->>Client: Apply the fill, track the remaining open quantity, and store the execution sequence
```

## Client successfully cancels an existing order

When a price event comes in a client may choose to cancel an existing order at random to free up some of its inventory for other trades.

```mermaid
sequenceDiagram
    participant MarketData as Price Aggregator
    participant Cache as Price Cache
    participant Gateway as Market Data Gateway
    participant Client as Trading Client
    participant OrderGateway as Orderbook Gateway
    participant Orderbook as Instrument Orderbook
    participant Log as Execution Log

    Client->>Gateway: Subscribe to the instrument
    Client->>OrderGateway: Log in with client ID and last execution sequence
    OrderGateway-->>Client: Login accepted
    Note over Client,Orderbook: The client has an open order from an earlier placement
    MarketData->>Cache: Publish the instrument
    Cache-->>Gateway: Publish received
    Gateway->>Cache: Read the latest price
    Gateway->>Client: Price update for the subscribed instrument
    Client->>Client: Select one open order at random
    Client->>OrderGateway: Cancel the order
    OrderGateway->>Orderbook: Submit the cancellation
    Orderbook->>Orderbook: Remove the remaining quantity
    Orderbook->>Log: Publish the cancellation
    Log-->>OrderGateway: Cancellation
    OrderGateway-->>Client: Cancellation confirmed
    Client->>Client: Release the reserved balance, remove the open order, and store the execution sequence
```

## Client requests are rejected before login

An orderbook gateway connection does not accept trading requests until the client has successfully logged in. Placement and cancellation attempts both receive the same not-logged-in response.

```mermaid
sequenceDiagram
    participant Client as Trading Client
    participant OrderGateway as Orderbook Gateway

    Client->>OrderGateway: Open WebSocket connection without a login sequence
    Client->>OrderGateway: Place a limit order
    OrderGateway-->>Client: Login rejected: not logged in
    Client->>OrderGateway: Cancel an order
    OrderGateway-->>Client: Login rejected: not logged in
```
