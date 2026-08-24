default: build-price-generator build-market-data-provider build-client build-orderbook

build: build-price-generator build-market-data-provider build-client build-orderbook

# Rust
build-price-generator:
	docker build -f docker/Dockerfile.price-generator -t orderbook-trading-price-generator:latest .

build-market-data-provider:
	docker build -f docker/Dockerfile.market-data-provider -t orderbook-trading-market-data-provider:latest .

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
