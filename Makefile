.PHONY: dev build build-dev up down logs clean help test test-backend test-frontend setup-hosts run-reference-stations gen-pics run

# Host UID/GID propagated into the dev frontend container so files written
# into the ./frontend bind mount (the regenerated src/api/generated client)
# are owned by the host developer, not root. UID/GID are not exported by the
# shell by default, so compute them explicitly here and export to compose.
export HOST_UID := $(shell id -u)
export HOST_GID := $(shell id -g)

# Default target
help:
	@echo "MESA Tool - Docker Commands"
	@echo ""
	@echo "Usage: make [target]"
	@echo ""
	@echo "Development:"
	@echo "  dev          Build and start development environment (hot reload)"
	@echo "  dev-build    Build development images only"
	@echo "  dev-up       Start development environment (no build)"
	@echo "  dev-down     Stop development environment"
	@echo "  dev-logs     View development logs"
	@echo ""
	@echo "Local (non-Docker):"
	@echo "  build        Build workspace + frontend (native, no Docker)"
	@echo "  run          Build, then run web_server (serves API + frontend at :8000)"
	@echo ""
	@echo "Testing:"
	@echo "  test          Run frontend tests"
	@echo "  test-frontend Run frontend vitest tests"
	@echo ""
	@echo "General:"
	@echo "  setup-hosts             Add 1815.2tests.local to /etc/hosts (requires sudo)"
	@echo "  run-reference-stations  Run reference outstation (background) + control station (foreground)"
	@echo "  gen-pics                Build pics-validator and generate JSON profiles from data/profiles/*.xlsx"
	@echo "  clean                   Stop all containers and remove volumes"
	@echo "  help                    Show this help message"

# Add 1815.2tests.local to /etc/hosts if not present
setup-hosts:
	@if ! grep -q "1815.2tests.local" /etc/hosts; then \
		echo "Adding 1815.2tests.local to /etc/hosts (requires sudo)..."; \
		echo "127.0.0.1  1815.2tests.local" | sudo tee -a /etc/hosts > /dev/null; \
		echo "Added 1815.2tests.local to /etc/hosts"; \
	else \
		echo "1815.2tests.local already in /etc/hosts"; \
	fi

# Development targets
dev: dev-build dev-up

dev-build:
	docker compose -f docker-compose.dev.yml build

dev-up:
	docker compose -f docker-compose.dev.yml up -d
	@echo ""
	@echo "=========================================================="
	@echo "  MESA dev stack up (Poem-only after #259)"
	@echo ""
	@echo "  Frontend:               http://localhost:3000"
	@echo "  Frontend /api/* proxy:  Poem web_server (mesa-backend-dev:8000)"
	@echo ""
	@echo "  Direct backend (bypass proxy):"
	@echo "    Poem:                 http://localhost:8025"
	@echo "=========================================================="
	@echo ""

dev-down:
	docker compose -f docker-compose.dev.yml down

dev-logs:
	docker compose -f docker-compose.dev.yml logs -f

build:
	cargo build --workspace
	cd frontend && npm install && npm run build

run: build
	@echo ""
	@echo "=========================================================="
	@echo "  MESA local stack (native, no Docker)"
	@echo ""
	@echo "  Webserver:                      http://localhost:8000"
	@echo "  Frontend (served by webserver): http://localhost:8000/"
	@echo "  OpenAPI:                        http://localhost:8000/openapi.json"
	@echo "=========================================================="
	@echo ""
	cargo run -p web_server

# Testing targets
test: test-frontend

test-frontend:
	docker compose -f docker-compose.dev.yml exec frontend-dev npm test

# Run reference stations locally (outstation in background, control station in foreground).
# The outstation is killed automatically when the control station exits (trap on EXIT).
run-reference-stations:
	cargo build -p reference-outstation -p reference-control-station
	@$(CURDIR)/target/debug/reference-outstation --local 127.0.0.1:20000 --profile $(CURDIR)/data/profiles/full.json < /dev/null & \
	OUTSTATION_PID=$$!; \
	trap "kill $$OUTSTATION_PID 2>/dev/null" EXIT; \
	trap "exit" INT TERM; \
	$(CURDIR)/target/debug/reference-control-station --outstation-ip 127.0.0.1 --outstation-port 20000 --profile $(CURDIR)/data/profiles/full.json; \
	wait $$OUTSTATION_PID 2>/dev/null || true

# Generate JSON profiles from xlsx files in data/profiles/
gen-pics:
	cargo build -p pics-validator
	@for xlsx in $(CURDIR)/data/profiles/*.xlsx; do \
		json="$${xlsx%.xlsx}.json"; \
		echo "Generating $$json"; \
		$(CURDIR)/target/debug/pics-validator "$$xlsx" "$$json"; \
	done

# Clean up
clean:
	docker compose -f docker-compose.dev.yml down -v --remove-orphans 2>/dev/null || true

install:
	curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
	~/.cargo/bin/cargo build
	cd frontend && npm install && npm run build
