# ─── Schedula Makefile ────────────────────────────────────────────────────────
#
# The hub server is bundled as a Tauri sidecar and starts automatically when
# Hub Mode is enabled inside the app — no need to run it separately.
#
# Development targets:
#   make dev            - Tauri app + license server (recommended)
#   make dev-all        - Tauri app + license server + landing page
#   make tauri          - Tauri desktop app only
#   make license        - License server only      (port 8080)
#   make hub            - Hub server only          (port 7878, for standalone testing)
#   make landing        - Landing page dev server  (port 3001)
#
# Build targets:
#   make build              - Build Tauri app for release
#   make build-hub-sidecar  - Build hub binary and copy into src-tauri/binaries/
#   make build-hub          - Build hub server binary for release
#   make build-license      - Build license server for release
#   make build-all          - Build everything for release
#
# Other:
#   make test           - Run all tests
#   make check          - cargo check all Rust crates
#   make keys-gen       - Generate a new RS256 key pair in license-server/keys/
#   make deploy-license - Deploy license server to Fly.io
#   make install        - Install Node dependencies
#   make clean          - Clean all build artefacts
#   make help           - Show this help

.PHONY: dev dev-all tauri hub license landing \
        build build-hub build-hub-sidecar build-license build-all \
        test test-tauri test-hub test-license check \
        keys-gen deploy-license install clean help

# ── Config ────────────────────────────────────────────────────────────────────

HUB_PORT      ?= 7878
HUB_DB        ?= ./schedula-hub.db
LICENSE_PORT  ?= 8080
LICENSE_DB    ?= ./schedula-license.db
LICENSE_DIR   := license-server
ADMIN_KEY     ?= dev-admin-key
LANDING_PORT  ?= 3001
LANDING_DIR   := landing

# ── Inline commands ───────────────────────────────────────────────────────────

_TAURI_CMD   = npm run tauri dev
_HUB_CMD     = cargo run --manifest-path hub-server/Cargo.toml -- \
                 --port $(HUB_PORT) --db-path $(HUB_DB)
_LICENSE_CMD = SCHEDULA_ADMIN_KEY=$(ADMIN_KEY) \
               cargo run --manifest-path $(LICENSE_DIR)/Cargo.toml -- \
                 --port $(LICENSE_PORT) --db-path $(LICENSE_DB) \
                 --private-key  $(LICENSE_DIR)/keys/private.pem \
                 --public-key   $(LICENSE_DIR)/keys/public.pem
_LANDING_CMD = python3 -m http.server $(LANDING_PORT) --directory $(LANDING_DIR) 2>/dev/null || \
               python  -m SimpleHTTPServer $(LANDING_PORT)

# ── Dev targets ───────────────────────────────────────────────────────────────

## Tauri app + license server  (hub runs inside the app via sidecar)
dev: _check_node _check_license_keys
	@echo "Starting:"
	@echo "  Tauri app      → launched by Vite"
	@echo "  License server → http://localhost:$(LICENSE_PORT)"
	@echo "  Admin UI       → http://localhost:$(LICENSE_PORT)/admin?key=$(ADMIN_KEY)"
	@echo ""
	npx --yes concurrently \
	  --names "TAURI,LICENSE" \
	  --prefix-colors "cyan,magenta" \
	  "$(_TAURI_CMD)" \
	  "$(_LICENSE_CMD)"

## Tauri app + license server + landing page
dev-all: _check_node _check_license_keys
	@echo "Starting:"
	@echo "  Tauri app      → launched by Vite"
	@echo "  License server → http://localhost:$(LICENSE_PORT)"
	@echo "  Landing page   → http://localhost:$(LANDING_PORT)"
	@echo "  Admin UI       → http://localhost:$(LICENSE_PORT)/admin?key=$(ADMIN_KEY)"
	@echo ""
	npx --yes concurrently \
	  --names "TAURI,LICENSE,LANDING" \
	  --prefix-colors "cyan,magenta,green" \
	  "$(_TAURI_CMD)" \
	  "$(_LICENSE_CMD)" \
	  "$(_LANDING_CMD)"

## Tauri desktop app only  (Vite frontend + Rust backend)
tauri: _check_node
	npm run tauri dev

## License server only  (port $(LICENSE_PORT))
## Override admin key: make license ADMIN_KEY=mysecret
license: _check_license_keys
	@echo "License server → http://localhost:$(LICENSE_PORT)"
	@echo "Admin UI       → http://localhost:$(LICENSE_PORT)/admin?key=$(ADMIN_KEY)"
	$(_LICENSE_CMD)

## Hub server only  (port $(HUB_PORT)) — useful for standalone testing
## In normal dev the hub starts automatically via the Tauri sidecar
hub:
	@echo "Hub server → http://localhost:$(HUB_PORT)"
	$(_HUB_CMD)

## Landing page dev server  (port $(LANDING_PORT))
landing:
	@echo "Landing page → http://localhost:$(LANDING_PORT)"
	@if command -v python3 > /dev/null 2>&1; then \
	  python3 -m http.server $(LANDING_PORT) --directory $(LANDING_DIR); \
	elif command -v python > /dev/null 2>&1; then \
	  cd $(LANDING_DIR) && python -m SimpleHTTPServer $(LANDING_PORT); \
	else \
	  echo "python3 not found — open $(LANDING_DIR)/index.html directly"; \
	  exit 1; \
	fi

# ── Build targets ─────────────────────────────────────────────────────────────

## Build Tauri app for release (run build-hub-sidecar first)
build: _check_node
	npm run tauri build

## Build hub sidecar binary and copy into src-tauri/binaries/
## Must be run before `make build` or `make tauri` on a fresh checkout
build-hub-sidecar:
	bash scripts/build-hub-sidecar.sh

## Build hub server binary standalone (release)
build-hub:
	cargo build --release --manifest-path hub-server/Cargo.toml
	@echo "Binary → hub-server/target/release/schedula-hub"

## Build license server binary (release)
build-license:
	cargo build --release --manifest-path $(LICENSE_DIR)/Cargo.toml
	@echo "Binary → $(LICENSE_DIR)/target/release/schedula-license"

## Build everything for release
build-all: build-hub-sidecar build build-license
	@echo ""
	@echo "All builds complete:"
	@echo "  Desktop app    → src-tauri/target/release/"
	@echo "  License server → license-server/target/release/schedula-license"
	@echo "  Landing page   → landing/index.html (static, no build needed)"

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

# ── Setup & Deploy ────────────────────────────────────────────────────────────

## Generate a new RS256 key pair in license-server/keys/
## WARNING: invalidates all previously issued JWTs
keys-gen:
	@echo "Generating RS256 key pair in $(LICENSE_DIR)/keys/"
	@mkdir -p $(LICENSE_DIR)/keys
	openssl genrsa -out $(LICENSE_DIR)/keys/private.pem 2048
	openssl rsa -in $(LICENSE_DIR)/keys/private.pem -pubout -out $(LICENSE_DIR)/keys/public.pem
	@echo "Done — copy the public key into hub-server/keys/license_public.pem:"
	@echo ""
	@cat $(LICENSE_DIR)/keys/public.pem

## Deploy license server to Fly.io
## First-time setup:
##   fly launch --no-deploy --name schedula-license  (inside license-server/)
##   fly volumes create license_data --size 1 --region sin
##   fly secrets set SCHEDULA_ADMIN_KEY=... STRIPE_SECRET_KEY=... etc.
deploy-license: _check_license_keys
	@command -v fly > /dev/null 2>&1 || \
	  (echo "Error: flyctl not found — https://fly.io/docs/flyctl/install/" && exit 1)
	cd $(LICENSE_DIR) && fly deploy

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
	@echo "  ─────────────────────────────────────────────────────────────────────"
	@echo ""
	@echo "  Note: the hub server is a Tauri sidecar — it starts inside the app"
	@echo "  when Hub Mode is enabled. No need to run it separately in dev."
	@echo ""
	@echo "  Development"
	@echo "    make dev        Tauri app + license server          (recommended)"
	@echo "    make dev-all    Tauri app + license server + landing"
	@echo "    make tauri      Tauri app only"
	@echo "    make license    License server only       (port $(LICENSE_PORT))"
	@echo "    make hub        Hub server only           (port $(HUB_PORT), standalone)"
	@echo "    make landing    Landing page dev server   (port $(LANDING_PORT))"
	@echo ""
	@echo "  Build"
	@echo "    make build              Build Tauri release app"
	@echo "    make build-hub-sidecar  Build hub + copy to src-tauri/binaries/ (run first)"
	@echo "    make build-hub          Build hub server standalone"
	@echo "    make build-license      Build license server"
	@echo "    make build-all          Build everything"
	@echo ""
	@echo "  Testing"
	@echo "    make test          Run all tests"
	@echo "    make test-tauri    src-tauri tests"
	@echo "    make test-hub      hub-server tests"
	@echo "    make test-license  license-server tests"
	@echo "    make check         cargo check all crates (fast)"
	@echo ""
	@echo "  Setup"
	@echo "    make keys-gen        Generate RS256 key pair (invalidates all JWTs)"
	@echo "    make deploy-license  Deploy license server to Fly.io"
	@echo "    make install         Install Node dependencies"
	@echo "    make clean           Remove all build artefacts"
	@echo ""
	@echo "  Variables  (override on the CLI, e.g. make dev ADMIN_KEY=secret)"
	@echo "    HUB_PORT      $(HUB_PORT)           ADMIN_KEY    $(ADMIN_KEY)"
	@echo "    HUB_DB        $(HUB_DB)    LICENSE_PORT $(LICENSE_PORT)"
	@echo "    LICENSE_DB    $(LICENSE_DB)    LANDING_PORT $(LANDING_PORT)"
	@echo ""
