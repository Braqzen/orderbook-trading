default: build-price-generator build-price-aggregator build-market-data-gateway build-client build-orderbook

build: build-price-generator build-price-aggregator build-market-data-gateway build-client build-orderbook

# Rust
build-price-generator:
	docker build -f docker/Dockerfile.price-generator -t orderbook-trading-price-generator:latest .

build-price-aggregator:
	docker build -f docker/Dockerfile.price-aggregator -t orderbook-trading-price-aggregator:latest .

build-market-data-gateway:
	docker build -f docker/Dockerfile.market-data-gateway -t orderbook-trading-market-data-gateway:latest .

build-client:
	docker build -f docker/Dockerfile.client -t orderbook-trading-client:latest .

build-orderbook:
	docker build -f docker/Dockerfile.orderbook -t orderbook-trading-orderbook:latest .

# Docker Compose Commands
start clients="10":
	docker compose -f docker/docker-compose.yml up -d --scale client={{clients}}
	@echo Grafana: http://localhost:3000/dashboards

stop:
	docker compose -f docker/docker-compose.yml down

remove:
	docker compose -f docker/docker-compose.yml down -v
