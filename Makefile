# Build artifacts go to the repository's target/ unless CARGO_TARGET_DIR is set; exporting it
# overrides a machine-wide cargo `target-dir` so the wasm and native paths below always exist.
CARGO_TARGET_DIR ?= $(CURDIR)/target
export CARGO_TARGET_DIR

# Koplik build targets. Cargo runs through the machine resource governor when present.
CARGO := $(shell if [ -x $(HOME)/.rsi/bin/cargo-slot ]; then echo $(HOME)/.rsi/bin/cargo-slot cargo; else echo cargo; fi)

.PHONY: check test schema wasm determinism wasm-benchmark web-test pipeline serve publish

check:
	$(CARGO) check --workspace --all-targets

test:
	$(CARGO) test --workspace

# Regenerate the committed JSON Schema for koplik-contracts (then commit the result).
schema:
	KOPLIK_REGEN_SCHEMA=1 $(CARGO) test -p koplik-contracts --test schema --test schema_v2 --test schema_v3

wasm:
	$(CARGO) build --locked --release --target wasm32-unknown-unknown -p koplik-wasm --lib
	@set -eu; version=$$(node tools/wasm/lock-version.mjs); \
	cli="target/tools/bin/wasm-bindgen"; \
	if [ ! -x "$$cli" ] || [ "$$($$cli --version)" != "wasm-bindgen $$version" ]; then \
		$(CARGO) install --locked --root target/tools wasm-bindgen-cli --version "$$version"; \
	fi; \
	"$$cli" --target web --out-dir pkg/web $(CARGO_TARGET_DIR)/wasm32-unknown-unknown/release/koplik_wasm.wasm; \
	"$$cli" --target nodejs --out-dir pkg/node $(CARGO_TARGET_DIR)/wasm32-unknown-unknown/release/koplik_wasm.wasm

determinism: wasm
	$(CARGO) build --locked --release -p koplik-wasm --bin trajectory-native
	node tools/wasm/determinism.mjs

wasm-benchmark: wasm
	node tools/wasm/benchmark.mjs

# Install web dependencies from the lockfile when it changes (works from a clean checkout).
web/node_modules/.package-lock.json: web/package-lock.json
	cd web && npm ci --no-audit --no-fund

web-test: web/node_modules/.package-lock.json
	cd web && npm test

pipeline:
	@echo "make pipeline: not implemented yet" >&2; exit 1

serve: web/node_modules/.package-lock.json
	cd web && npm run build && npm run preview

publish:
	@echo "make publish: not implemented yet" >&2; exit 1
