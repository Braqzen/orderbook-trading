# Decisions

### Client

The client intentionally uses a weighted random choice instead of a trading strategy because strategies are not (currently) implemented.

```mermaid
flowchart TD
    Price["Receive a new price for a subscribed instrument"] --> Action{"Choose a trading action"}

    Action -->|"50% of updates"| Nothing["Take no action"]
    Action -->|"40% of updates"| NewOrder["Create a buy or sell limit order<br/>at the latest price with a random<br/>quantity inside the configured limits"]
    NewOrder --> Place["Request placement from that instrument's orderbook"]

    Action -->|"10% of updates"| OpenOrders{"Does the client have an open order?"}
    OpenOrders -->|"No"| Nothing
    OpenOrders -->|"Yes"| Select["Select one open order at random"]
    Select --> Cancel["Request cancellation from that instrument's orderbook"]
```

### Orderbook Service

The orderbook sends an explicit response for every rejection, accepted remainder, trade, or cancellation result.

```mermaid
flowchart TD
    Request{"Request received from a trading client"}

    Request -->|"Place a limit order"| Risk{"Does the order pass<br/>the risk policy?"}
    Risk -->|"No"| Rejected["Send Order Rejected<br/>to the requesting client"]
    Risk -->|"Yes"| Trade["Match and execute trades against<br/>the best-priced orders on the opposite side"]
    Trade --> Remaining{"How much of the new order remains?"}

    Remaining -->|"Full quantity"| StoreFull["Store the full order<br/>for a future match"]
    StoreFull --> AcceptedFull["Send Order Accepted<br/>to the requesting client"]

    Remaining -->|"No quantity"| Complete["Send Trade results to every affected client"]
    Remaining -->|"Partial quantity"| StoreRest["Store the remaining quantity<br/>for a future match"]
    StoreRest --> PartialResponses["Send Trade results to every affected client and an Order Accepted to indicate resting order"]

    Request -->|"Cancel an open order"| Found{"Is that order still<br/>in the orderbook?"}
    Found -->|"Yes"| Remove["Remove the order"]
    Remove --> Cancelled["Send Cancellation Confirmed<br/>to the requesting client"]
    Found -->|"No"| CancelRejected["Send Cancellation Rejected<br/>to the requesting client"]
```
