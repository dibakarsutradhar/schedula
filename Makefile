# ─── Schedula Makefile ────────────────────────────────────────────────────────
#
# Targets:
#   make dev            - Run Tauri app + hub server concurrently (most common)
#   make dev-all        - Run Tauri app + hub server + landing page server
#   make tauri          - Tauri desktop app only  (Vite + Rust backend)
#   make hub            - Hub server only          (Axum HTTP + WebSocket)
#   make license        - License server only      (Axum HTTP, port 8080)
#   make landing        - Landing page dev server  (Python simple HTTP)
#
#   make build          - Build Tauri app for release
#   make build-hub      - Build hub server for release
#   make build-license  - Build license server for release
#   make build-all      - Build everything for release
#
#   make test           - Run all tests
#   make test-tauri     - Run src-tauri tests only
#   make test-hub       - Run hub-server tests only
#   make test-license   - Run license-server tests only
#   make check          - cargo check all Rust crates (fast compile check)
#
#   make keys-gen       - Generate a new RS256 key pair in license-server/keys/
#   make install        - Install Node dependencies
#   make clean          - Clean all build artefacts
#   make help           - Show this help

.PHONY: dev dev-all tauri hub license landing \
        build build-hub build-license build-all \
        test test-tauri test-hub test-license check \
        keys-gen install clean help

# ── Config ────────────────────────────────────────────────────────────────────

HUB_PORT      ?= 7878
HUB_DB        ?= ./schedula-hub.db
LICENSE_PORT  ?= 8080
LICENSE_DB    ?= ./schedula-license.db
LICENSE_DIR   := license-server
ADMIN_KEY     ?= dev-admin-key
LANDING_PORT  ?= 3001
LANDING_DIR   := landing

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

## Run Tauri app + hub server + license server concurrently (full stack)
dev-full: _check_node
ifdef CONCURRENTLY
	npx --yes concurrently \
	  --names "TAURI,HUB,LICENSE" \
	  --prefix-colors "cyan,yellow,magenta" \
	  --kill-others-on-fail \
	  "npm run tauri dev" \
	  "$(MAKE) hub" \
	  "$(MAKE) license"
else
	@$(MAKE) hub &
	@$(MAKE) license &
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

## Start license server (Axum HTTP on port $(LICENSE_PORT))
## Requires ADMIN_KEY env var (or override: make license ADMIN_KEY=secret)
license: _check_license_keys
	@echo "Starting license server on http://localhost:$(LICENSE_PORT)"
	@echo "  Admin UI → http://localhost:$(LICENSE_PORT)/admin?key=$(ADMIN_KEY)"
	SCHEDULA_ADMIN_KEY=$(ADMIN_KEY) \
	cargo run --manifest-path $(LICENSE_DIR)/Cargo.toml -- \
	  --port $(LICENSE_PORT) \
	  --db-path $(LICENSE_DB) \
	  --private-key $(LICENSE_DIR)/keys/private.pem \
	  --public-key  $(LICENSE_DIR)/keys/public.pem

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

## Build license server binary (release)
build-license:
	cargo build --release --manifest-path $(LICENSE_DIR)/Cargo.toml
	@echo "Binary → $(LICENSE_DIR)/target/release/schedula-license"

## Build everything for release
build-all: build build-hub build-license
	@echo ""
	@echo "All builds complete:"
	@echo "  Desktop app     → src-tauri/target/release/"
	@echo "  Hub server      → hub-server/target/release/schedula-hub"
	@echo "  License server  → license-server/target/release/schedula-license"
	@echo "  Landing page    → landing/index.html (static, no build needed)"

# ── Test targets ──────────────────────────────────────────────────────────────

## Run all tests
test: test-tauri test-hub test-license

## Run src-tauri tests
test-tauri:
	cargo test --manifest-path src-tauri/Cargo.toml

## Run hub-server tests
test-hub:
	cargo test --manifest-path hub-server/Cargo.toml

## Run license-server tests
test-license:
	cargo test --manifest-path $(LICENSE_DIR)/Cargo.toml

## cargo check all Rust crates (fast compile verification, no binary produced)
check:
	cargo check --manifest-path src-tauri/Cargo.toml
	cargo check --manifest-path hub-server/Cargo.toml
	cargo check --manifest-path $(LICENSE_DIR)/Cargo.toml

# ── Setup ─────────────────────────────────────────────────────────────────────

## Generate a new RS256 key pair in license-server/keys/
## WARNING: replaces existing keys — all issued JWTs will become invalid
keys-gen:
	@echo "Generating RS256 key pair in $(LICENSE_DIR)/keys/"
	@mkdir -p $(LICENSE_DIR)/keys
	openssl genrsa -out $(LICENSE_DIR)/keys/private.pem 2048
	openssl rsa -in $(LICENSE_DIR)/keys/private.pem -pubout -out $(LICENSE_DIR)/keys/public.pem
	@echo "Done — update hub-server/keys/license_public.pem with the new public key:"
	@echo ""
	@cat $(LICENSE_DIR)/keys/public.pem

## Install Node dependencies
install:
	npm install

# ── Cleanup ───────────────────────────────────────────────────────────────────

## Remove all build artefacts
clean:
	rm -rf node_modules dist
	cargo clean --manifest-path src-tauri/Cargo.toml
	cargo clean --manifest-path hub-server/Cargo.toml
	cargo clean --manifest-path $(LICENSE_DIR)/Cargo.toml
	@echo "Clean complete."

# ── Internal helpers ──────────────────────────────────────────────────────────

_check_node:
	@command -v node > /dev/null 2>&1 || \
	  (echo "Error: Node.js not found. Run 'make install' first." && exit 1)
	@test -d node_modules || \
	  (echo "node_modules missing — running npm install first..." && npm install)

_check_license_keys:
	@test -f $(LICENSE_DIR)/keys/private.pem || \
	  (echo "Error: $(LICENSE_DIR)/keys/private.pem not found. Run 'make keys-gen' first." && exit 1)
	@test -f $(LICENSE_DIR)/keys/public.pem || \
	  (echo "Error: $(LICENSE_DIR)/keys/public.pem not found. Run 'make keys-gen' first." && exit 1)

# ── Help ──────────────────────────────────────────────────────────────────────

help:
	@echo ""
	@echo "  Schedula — available make targets"
	@echo "  ─────────────────────────────────────────────────────────────"
	@echo ""
	@echo "  Development"
	@echo "    make dev           Tauri app + hub server (recommended)"
	@echo "    make dev-all       Tauri app + hub server + landing page"
	@echo "    make dev-full      Tauri app + hub server + license server"
	@echo "    make tauri         Tauri desktop app only"
	@echo "    make hub           Hub server only          (port $(HUB_PORT))"
	@echo "    make license       License server only      (port $(LICENSE_PORT))"
	@echo "    make landing       Landing page dev server  (port $(LANDING_PORT))"
	@echo ""
	@echo "  Build"
	@echo "    make build         Build Tauri app for release"
	@echo "    make build-hub     Build hub server binary for release"
	@echo "    make build-license Build license server binary for release"
	@echo "    make build-all     Build all targets"
	@echo ""
	@echo "  Testing"
	@echo "    make test          Run all tests"
	@echo "    make test-tauri    Run src-tauri tests"
	@echo "    make test-hub      Run hub-server tests"
	@echo "    make test-license  Run license-server tests"
	@echo "    make check         cargo check all Rust crates (fast)"
	@echo ""
	@echo "  Keys"
	@echo "    make keys-gen      Generate new RS256 key pair (invalidates all JWTs!)"
	@echo ""
	@echo "  Other"
	@echo "    make install       Install Node dependencies"
	@echo "    make clean         Remove all build artefacts"
	@echo ""
	@echo "  Variables (override on the command line, e.g. HUB_PORT=9000 make dev)"
	@echo "    HUB_PORT      Hub server port          (default: $(HUB_PORT))"
	@echo "    HUB_DB        Hub server DB path        (default: $(HUB_DB))"
	@echo "    LICENSE_PORT  License server port       (default: $(LICENSE_PORT))"
	@echo "    LICENSE_DB    License server DB path    (default: $(LICENSE_DB))"
	@echo "    ADMIN_KEY     License server admin key  (default: $(ADMIN_KEY))"
	@echo "    LANDING_PORT  Landing page port         (default: $(LANDING_PORT))"
	@echo ""
