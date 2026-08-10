default: build-generator build-market-feed

# Rust
build-generator:
	docker buildx bake --load -f docker/build.hcl generator

build-market-feed:
	docker buildx bake --load -f docker/build.hcl market-feed

# Docker Compose Commands
run:
	docker compose -f docker/docker-compose.yml up -d
	@echo Grafana: http://localhost:3000/dashboards

stop:
	docker compose -f docker/docker-compose.yml down

clean:
	docker compose -f docker/docker-compose.yml down -v
