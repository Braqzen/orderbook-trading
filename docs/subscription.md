# Subscriptions

## Market data provider

One WebSocket carries subscription changes and price updates.

### Subscribe

The client sends the instruments it wants prices for. An instrument already in that client's subscription is rejected and every other instrument is added. The client is informed about the result of all instruments.

```mermaid
flowchart TD
    Subscribe["Client sends a subscribe request"] --> Instruments{"Does the request<br/>name any instruments?"}

    Instruments -->|"No"| Generic["Send Rejected"]
    Instruments -->|"Yes"| Split["Split the instruments"]

    Split --> New["Instruments not yet subscribed"]
    Split --> Existing["Instruments already subscribed"]

    New --> Outcome{"Which instruments were added?"}
    Existing --> Outcome

    Outcome -->|"All of them"| Subscribed["Send Subscribed<br/>with those instruments"]
    Outcome -->|"Some of them"| Partial["Send Partially Subscribed<br/>with the added instruments<br/>and the rejected instruments"]
    Outcome -->|"None of them"| Rejected["Send Subscription Rejected<br/>with the rejected instruments"]
```

### Unsubscribe

The client sends the instruments it no longer wants. An instrument that is not in that client's subscription is rejected and every other instrument is removed. The client is informed about the result of all instruments.

```mermaid
flowchart TD
    Unsubscribe["Client sends an unsubscribe request"] --> Instruments{"Does the request<br/>name any instruments?"}

    Instruments -->|"No"| Generic["Send Rejected"]
    Instruments -->|"Yes"| Split["Split the instruments"]

    Split --> Current["Instruments currently subscribed"]
    Split --> Missing["Instruments not subscribed"]

    Current --> Outcome{"Which instruments were removed?"}
    Missing --> Outcome

    Outcome -->|"All of them"| Unsubscribed["Send Unsubscribed<br/>with those instruments"]
    Outcome -->|"Some of them"| Partial["Send Partially Unsubscribed<br/>with the removed instruments<br/>and the rejected instruments"]
    Outcome -->|"None of them"| Rejected["Send Subscription Rejected<br/>with the rejected instruments"]
```

### Price update

When the price generator sends an update for an instrument the provider checks which clients should receive the update.

> Note: All connected clients receive updates through 1 price channel and filter based on instrument subscriptions. This is fine but at larger scales (more instruments, more clients) we'd lean toward 1 cache containing the latest price per instrument with a TTL and when a client is connected to a gateway and subscribed to some instrument it would receive those events.

```mermaid
flowchart TD
    Price["Provider receives a price<br/>for an instrument"] --> Subscribed{"Is this client subscribed<br/>to that instrument?"}

    Subscribed -->|"Yes"| Send["Send Price<br/>with the instrument and value"]
    Subscribed -->|"No"| Drop["Send nothing"]
```
