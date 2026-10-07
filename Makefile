# Build artifacts go to the repository's target/ unless CARGO_TARGET_DIR is set; exporting it
# overrides a machine-wide cargo `target-dir` so the wasm and native paths below always exist.
CARGO_TARGET_DIR ?= $(CURDIR)/target
export CARGO_TARGET_DIR

# Koplik build targets. Cargo runs through the machine resource governor when present.
CARGO := $(shell if [ -x $(HOME)/.rsi/bin/cargo-slot ]; then echo $(HOME)/.rsi/bin/cargo-slot cargo; else echo cargo; fi)

PUBLISH_REMOTE ?= origin
PUBLISH_DRY_RUN ?= 0
export PUBLISH_REMOTE PUBLISH_DRY_RUN

.PHONY: check test schema wasm determinism wasm-benchmark web-test pipeline pipeline-fixtures serve publish install-refresh-timer refresh-test pipeline-release

check:
	$(CARGO) fetch --locked
	tools/offline-test.sh $(CARGO) check --offline --workspace --all-targets

test:
	tools/cargo-test.sh --workspace

# Regenerate the committed JSON Schema for koplik-contracts (then commit the result).
schema:
	KOPLIK_REGEN_SCHEMA=1 tools/cargo-test.sh -p koplik-contracts --test schema --test schema_v2 --test schema_v3 --test schema_v4 --test schema_v5

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
	tools/offline-test.sh node tools/wasm/determinism.mjs

wasm-benchmark: wasm
	node tools/wasm/benchmark.mjs

# Install web dependencies from the lockfile when it changes (works from a clean checkout).
web/node_modules/.package-lock.json: web/package-lock.json
	cd web && npm ci --no-audit --no-fund

web-test: wasm web/node_modules/.package-lock.json
	tools/offline-test.sh sh -eu -c 'node --test tools/offline-test.test.mjs tools/refresh/refresh.test.mjs tools/refresh/data.test.mjs; cd web; test_port="$$(node scripts/free-port.mjs)"; KOPLIK_TEST_PORT="$$test_port" KOPLIK_TEST_OUTPUT_DIR="test-results-$$test_port" npm test; cd ..; mkdir -p "$$CARGO_TARGET_DIR/publish-tests"; TMPDIR="$$CARGO_TARGET_DIR/publish-tests" node --test tools/publish.test.mjs'

# Live pipeline: ingest (network, identified by KOPLIK_CONTACT or the operator's default contact;
# a blank KOPLIK_CONTACT refuses) then validate, infer, forecast and build into web/public/data.
# Never falls back to fixtures.
pipeline:
	$(CARGO) run --locked --release -p koplik-pipeline -- all

# Offline pipeline from the committed real-byte fixtures in data/fixtures/ (their recorded
# provenance and retrieval times), into web/public/data. Used by QA and tests; no network.
pipeline-fixtures:
	$(CARGO) run --locked --release -p koplik-pipeline -- all --from-fixtures

serve: wasm web/node_modules/.package-lock.json
	cd web && KOPLIK_BASE_PATH=/ npm run build && KOPLIK_BASE_PATH=/ npm run preview

publish:
	node tools/publish.mjs

# Publication input selection: committed release store when present, else fixtures (offline).
pipeline-release:
	node tools/refresh/data.mjs . data/pipeline web/public/data

refresh-test:
	tools/offline-test.sh node --test tools/refresh/refresh.test.mjs tools/refresh/data.test.mjs

# Manager/operator only, after tier2 review; never run from an agent sandbox.
install-refresh-timer:
	tools/refresh/install.sh
