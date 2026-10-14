# One gate, run the same way here, in CI (.github/workflows/ci.yml calls these
# targets) and by the Stop hook (.claude/gates.json runs `make gate`).
#
# kbviewer-server embeds web/dist at compile time (rust-embed), so nothing Rust compiles
# without it. Every target that compiles depends on it and builds it the way CI does when
# it is missing; after changing web/src, `make web` rebuilds it.
CARGO ?= cargo
MUTANTS_DIFF ?= target/mutants.diff
FRONTEND := web/dist/index.html
# nextest runs each test in its own process, where ts-rs's export tests cannot share one
# file (.config/nextest.toml says why): they write here, and `bindings` writes the real one.
NEXTEST_BINDINGS_DIR ?= $(CURDIR)/target/ts-rs-nextest

.PHONY: gate web fmt lint test bindings deny vet machete bench bench-build mutants

gate: fmt lint test deny vet machete

$(FRONTEND):
	$(MAKE) web

web:
	cd web && npm ci && npm run build

fmt:
	$(CARGO) fmt --all --check

# clippy with every lint fatal, except arithmetic_side_effects and
# indexing_slicing, which fail only on lines this branch touches.
lint: $(FRONTEND)
	python3 scripts/clippy_ratchet.py -- --locked

test: $(FRONTEND)
	TS_RS_EXPORT_DIR=$(NEXTEST_BINDINGS_DIR) $(CARGO) nextest run --workspace --locked
	$(CARGO) test --workspace --locked --doc
	$(MAKE) bindings

# Regenerates web/src/api/types.ts in one process, as `cargo test` always has. CI then
# fails if it differs from what is committed.
bindings: $(FRONTEND)
	$(CARGO) test --locked -p kbviewer-core --lib export_bindings

deny:
	$(CARGO) deny --locked check

vet:
	$(CARGO) vet --locked

machete:
	$(CARGO) machete

# The search path over a generated ~1 MB vault (crates/kbviewer-core/benches/search.rs).
bench:
	$(CARGO) bench --locked -p kbviewer-core --bench search

# CI compiles the bench without running it, so it cannot rot unnoticed.
bench-build:
	$(CARGO) bench --locked --no-run -p kbviewer-core --bench search

# Minutes, not seconds: CI runs it on pull requests; run it by hand before one.
mutants: $(FRONTEND)
	mkdir -p $(dir $(MUTANTS_DIFF))
	git diff "$$(git merge-base HEAD origin/HEAD)" > $(MUTANTS_DIFF)
	$(CARGO) mutants --in-diff $(MUTANTS_DIFF)
