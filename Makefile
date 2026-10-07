# Koplik build targets. Cargo runs through the machine resource governor when present.
CARGO := $(shell if [ -x $(HOME)/.rsi/bin/cargo-slot ]; then echo $(HOME)/.rsi/bin/cargo-slot cargo; else echo cargo; fi)

.PHONY: check test schema wasm determinism web-test pipeline serve publish

check:
	$(CARGO) check --workspace --all-targets

test:
	$(CARGO) test --workspace

# Regenerate the committed JSON Schema for koplik-contracts (then commit the result).
schema:
	KOPLIK_REGEN_SCHEMA=1 $(CARGO) test -p koplik-contracts --test schema

wasm:
	@echo "make wasm: not implemented yet" >&2; exit 1

determinism:
	@echo "make determinism: not implemented yet" >&2; exit 1

web-test:
	cd web && npm test

pipeline:
	@echo "make pipeline: not implemented yet" >&2; exit 1

serve:
	cd web && npm run build && npm run preview

publish:
	@echo "make publish: not implemented yet" >&2; exit 1
