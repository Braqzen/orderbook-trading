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
