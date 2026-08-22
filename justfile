default: build-generator build-market-feed build-client build-orderbook

build: build-generator build-market-feed build-client build-orderbook

# Rust
build-generator:
	docker build -f docker/Dockerfile.generator -t orderbook-trading-generator:latest .

build-market-feed:
	docker build -f docker/Dockerfile.market-feed -t orderbook-trading-market-feed:latest .

build-client:
	docker build -f docker/Dockerfile.client -t orderbook-trading-client:latest .

build-orderbook:
	docker build -f docker/Dockerfile.orderbook -t orderbook-trading-orderbook:latest .

# Docker Compose Commands
run clients="10":
	docker compose -f docker/docker-compose.yml up -d --scale client={{clients}}
	@echo Grafana: http://localhost:3000/dashboards

stop:
	docker compose -f docker/docker-compose.yml down

clean:
	docker compose -f docker/docker-compose.yml down -v
