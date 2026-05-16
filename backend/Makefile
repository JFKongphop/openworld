# OpenWorld — Makefile shortcuts
# ─────────────────────────────────────────────────────────────────────────────
# make run            Start the API server (port 3000)
# make plan           Quick demo — Tokyo 5 days $1200 (waits for completion)
# make plan-paris     Paris 7 days $2500
# make plan-bali      Bali 8 days $1500
# make health         Check if server is up
# make check          cargo check
# make build          cargo build --release
# ─────────────────────────────────────────────────────────────────────────────

.PHONY: run run-cli plan plan-paris plan-bali health check build logs

SCRIPT := ./scripts/openworld.sh

# ── Server ────────────────────────────────────────────────────────────────────

run:
	cargo run --bin api

build:
	cargo build --release

check:
	cargo check

# ── CLI (no server needed) ────────────────────────────────────────────────────

cli:
	cargo run --bin travel -- "Bangkok" "TYO" 5 1200

cli-paris:
	cargo run --bin travel -- "Bangkok" "Paris" 7 2500

cli-bali:
	cargo run --bin travel -- "Bangkok" "Bali" 8 1500

cli-custom:
	cargo run --bin travel -- "Bangkok" "$(DEST)" $(DAYS) $(BUDGET)

# ── Trip Planning ─────────────────────────────────────────────────────────────

plan:
	@chmod +x $(SCRIPT)
	$(SCRIPT) plan "Tokyo" 5 1200

plan-paris:
	@chmod +x $(SCRIPT)
	$(SCRIPT) plan "Paris" 7 2500

plan-bali:
	@chmod +x $(SCRIPT)
	$(SCRIPT) plan "Bali" 8 1500

plan-custom:
	@chmod +x $(SCRIPT)
	$(SCRIPT) plan "$(DEST)" $(DAYS) $(BUDGET)

# ── Info ─────────────────────────────────────────────────────────────────────

health:
	@chmod +x $(SCRIPT)
	$(SCRIPT) health

# Usage hint
help:
	@echo ""
	@echo "  OpenWorld — Available targets:"
	@echo ""
	@echo "  make run            Start the API server"
	@echo "  make plan           Plan Tokyo 5d \$$1200 (full pipeline)"
	@echo "  make plan-paris     Plan Paris 7d \$$2500"
	@echo "  make plan-bali      Plan Bali 8d \$$1500"
	@echo "  make plan-custom DEST=\"Seoul\" DAYS=6 BUDGET=1800"
	@echo "  make health         Check server is running"
	@echo "  make build          Release build"
	@echo ""
