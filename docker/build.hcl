target "generator" {
  context    = "."
  dockerfile = "docker/Dockerfile.generator"
  tags       = ["orderbook-trading-generator:latest"]
}

target "market-feed" {
  context    = "."
  dockerfile = "docker/Dockerfile.market-feed"
  tags       = ["orderbook-trading-market-feed:latest"]
}

target "client" {
  context    = "."
  dockerfile = "docker/Dockerfile.client"
  tags       = ["orderbook-trading-client:latest"]
}

target "orderbook" {
  context    = "."
  dockerfile = "docker/Dockerfile.orderbook"
  tags       = ["orderbook-trading-orderbook:latest"]
}
