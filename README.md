# Orderbook Trading

## Overview

This project demonstrates a time-priority based orderbook where clients react to subscribed price events and make trading decisions.

Information:

- [Architecture Components](./docs/architecture.md): Overview of components and their relationships
- [End-to-end sequence diagrams](./docs/sequence.md): Example use cases from price generation to trading
- [Client & Orderbook decisions](./docs/decision.md): How services perform decisions based on events
- [Instrument Subscriptions](./docs/subscription.md): Client-Provider instrument subscription handling

## Usage

The services are run in `docker-compose` via `just` commands but you may use docker commands directly from the [justfile](./justfile).

### Build Services

This builds the price generator, market data provider, client and orderbook programmes.

```sh
just build
```

To build a specific service check the [justfile](./justfile).

### Start Services

The default starts with 10 clients and 5 orderbooks (1 for each instrument).

```sh
just start
```

To start with a different number of clients specify a number

```sh
just start 5
```

To view telemetry about the services navigate to `http://localhost:3000/dashboards`

### Stop Services

The services are in-memory however telemetry is persistent.

Stop and retain telemetry

```sh
just stop
```

Stop and delete telemetry

```sh
just remove
```
