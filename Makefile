# ─── Schedula Makefile ────────────────────────────────────────────────────────
#
# Targets:
#   make dev          - Run Tauri app + hub server concurrently (most common)
#   make dev-all      - Run Tauri app + hub server + landing page server
#   make tauri        - Tauri desktop app only  (Vite + Rust backend)
#   make hub          - Hub server only          (Axum HTTP + WebSocket)
#   make landing      - Landing page dev server  (Python simple HTTP)
#
#   make build        - Build Tauri app for release
#   make build-hub    - Build hub server for release
#   make build-all    - Build everything for release
#
#   make test         - Run all tests (Tauri unit tests + hub server tests)
#   make test-tauri   - Run src-tauri tests only
#   make test-hub     - Run hub-server tests only
#
#   make install      - Install Node dependencies
#   make clean        - Clean all build artefacts
#   make help         - Show this help

.PHONY: dev dev-all tauri hub landing \
        build build-hub build-all \
        test test-tauri test-hub \
        install clean help

# ── Config ────────────────────────────────────────────────────────────────────

HUB_PORT     ?= 7878
HUB_DB       ?= ./schedula-hub.db
LANDING_PORT ?= 3001
LANDING_DIR  := landing

# Detect OS for open-browser command
UNAME := $(shell uname -s)
ifeq ($(UNAME), Darwin)
  OPEN := open
else ifeq ($(UNAME), Linux)
  OPEN := xdg-open
else
  OPEN := start
endif

# Use npx concurrently if available, otherwise fall back to & (background jobs)
CONCURRENTLY := $(shell command -v npx 2>/dev/null && npx --yes concurrently --version > /dev/null 2>&1 && echo "npx concurrently" || echo "")

# ── Dev targets ───────────────────────────────────────────────────────────────

## Run Tauri desktop app + hub server concurrently
dev: _check_node
ifdef CONCURRENTLY
	npx --yes concurrently \
	  --names "TAURI,HUB" \
	  --prefix-colors "cyan,yellow" \
	  --kill-others-on-fail \
	  "npm run tauri dev" \
	  "$(MAKE) hub"
else
	@echo "Starting hub server in background..."
	@$(MAKE) hub &
	@echo "Starting Tauri app..."
	npm run tauri dev
endif

## Run Tauri app + hub server + landing page server concurrently
dev-all: _check_node
ifdef CONCURRENTLY
	npx --yes concurrently \
	  --names "TAURI,HUB,LANDING" \
	  --prefix-colors "cyan,yellow,green" \
	  --kill-others-on-fail \
	  "npm run tauri dev" \
	  "$(MAKE) hub" \
	  "$(MAKE) landing"
else
	@$(MAKE) hub &
	@$(MAKE) landing &
	npm run tauri dev
endif

## Start Tauri desktop app (Vite frontend + Rust backend)
tauri: _check_node
	npm run tauri dev

## Start hub server (Axum HTTP + WebSocket on port $(HUB_PORT))
hub:
	@echo "Starting hub server on http://localhost:$(HUB_PORT)"
	cargo run --manifest-path hub-server/Cargo.toml -- \
	  --port $(HUB_PORT) \
	  --db-path $(HUB_DB)

## Serve landing page on http://localhost:$(LANDING_PORT)
landing:
	@echo "Landing page → http://localhost:$(LANDING_PORT)"
	@if command -v python3 > /dev/null 2>&1; then \
	  python3 -m http.server $(LANDING_PORT) --directory $(LANDING_DIR); \
	elif command -v python > /dev/null 2>&1; then \
	  cd $(LANDING_DIR) && python -m SimpleHTTPServer $(LANDING_PORT); \
	else \
	  echo "python3 not found — install it or open $(LANDING_DIR)/index.html directly"; \
	  exit 1; \
	fi

# ── Build targets ─────────────────────────────────────────────────────────────

## Build Tauri desktop app (release)
build: _check_node
	npm run tauri build

## Build hub server binary (release)
build-hub:
	cargo build --release --manifest-path hub-server/Cargo.toml
	@echo "Binary → hub-server/target/release/schedula-hub"

## Build everything for release
build-all: build build-hub
	@echo ""
	@echo "All builds complete:"
	@echo "  Desktop app  → src-tauri/target/release/"
	@echo "  Hub server   → hub-server/target/release/schedula-hub"
	@echo "  Landing page → landing/index.html (static, no build needed)"

# ── Test targets ──────────────────────────────────────────────────────────────

## Run all tests
test: test-tauri test-hub

## Run src-tauri tests
test-tauri:
	cargo test --manifest-path src-tauri/Cargo.toml

## Run hub-server tests
test-hub:
	cargo test --manifest-path hub-server/Cargo.toml

# ── Setup ─────────────────────────────────────────────────────────────────────

## Install Node dependencies
install:
	npm install

# ── Cleanup ───────────────────────────────────────────────────────────────────

## Remove all build artefacts
clean:
	rm -rf node_modules dist
	cargo clean --manifest-path src-tauri/Cargo.toml
	cargo clean --manifest-path hub-server/Cargo.toml
	@echo "Clean complete."

# ── Internal helpers ──────────────────────────────────────────────────────────

_check_node:
	@command -v node > /dev/null 2>&1 || \
	  (echo "Error: Node.js not found. Run 'make install' first." && exit 1)
	@test -d node_modules || \
	  (echo "node_modules missing — running npm install first..." && npm install)

# ── Help ──────────────────────────────────────────────────────────────────────

help:
	@echo ""
	@echo "  Schedula — available make targets"
	@echo "  ─────────────────────────────────────────────────────────"
	@echo ""
	@echo "  Development"
	@echo "    make dev          Tauri app + hub server (recommended)"
	@echo "    make dev-all      Tauri app + hub server + landing page"
	@echo "    make tauri        Tauri desktop app only"
	@echo "    make hub          Hub server only  (port $(HUB_PORT))"
	@echo "    make landing      Landing page dev server (port $(LANDING_PORT))"
	@echo ""
	@echo "  Build"
	@echo "    make build        Build Tauri app for release"
	@echo "    make build-hub    Build hub server binary for release"
	@echo "    make build-all    Build all targets"
	@echo ""
	@echo "  Testing"
	@echo "    make test         Run all tests"
	@echo "    make test-tauri   Run src-tauri tests"
	@echo "    make test-hub     Run hub-server tests"
	@echo ""
	@echo "  Other"
	@echo "    make install      Install Node dependencies"
	@echo "    make clean        Remove all build artefacts"
	@echo ""
	@echo "  Variables (override with HUB_PORT=xxxx make dev)"
	@echo "    HUB_PORT     Hub server port        (default: $(HUB_PORT))"
	@echo "    HUB_DB       Hub server DB path      (default: $(HUB_DB))"
	@echo "    LANDING_PORT Landing page port       (default: $(LANDING_PORT))"
	@echo ""
