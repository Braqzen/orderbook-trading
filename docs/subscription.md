# Subscriptions

## Market data provider gateway

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

When the price generator sends an update for an instrument the aggregator stores the latest price in the cache and publishes a message. The gateway receives the message, reads that price and sends it to clients subscribed to that instrument.

```mermaid
flowchart TD
    Price["Aggregator receives price update"] --> Store["Store price in cache"]
    Store --> Publish["Aggregator publishes<br/>through the cache"]
    Publish --> Receive["Gateway receives the publish"]
    Receive --> Read["Gateway reads that price<br/>from the cache"]
    Read --> Send["Send Price<br/>to clients subscribed to that instrument"]
```
