# Publishing Orderbook Actions

When the orderbook performs an action, such as a trade, acceptance, rejection, or cancellation, it publishes a message to `Redpanda` (queues messages for replay).

The gateway reads that log and streams messages to connected clients. If a client is not connected, upon login, the gateway finds the messages from the sequence number the client provided and streams them in order.

## Reading from the start

Currently, the gateway must find which partition the client's data is in and then read the log from the start until it finds the first client record which contains their requested sequence number. This is not the internal position of the queue, it is a per client number identifying a specific action.
