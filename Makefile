# Canonical verb contract: humans, agents, and CI all use these verb names.
# Bodies are thin adapters over cargo. Run from this directory:  make verify
#
# Builds use --locked: Cargo.lock is committed, so everyone (and CI) compiles
# exactly the same dependency versions.

.PHONY: setup build typecheck lint fmt test test-affected integrity verify doctor install demo play ui playground-ui

setup:          ## idempotent bootstrap: the toolchain components the gates use
	rustup component add rustfmt clippy

build:          ## debug build → target/debug/hippo-task
	cargo build --locked

typecheck:      ## compile-check everything (incl. tests) without producing binaries
	cargo check --locked --all-targets

fmt:            ## format the code in place
	cargo fmt

lint:           ## formatting + clippy; warnings are errors, incl. the no-panic lints in Cargo.toml
	cargo fmt --check
	cargo clippy --locked --all-targets -- -D warnings

test:           ## the whole suite: model/fold/render unit tests, store, ops, CLI contract, docs
	cargo test --locked

test-affected:  ## fast subset (small crate: it's the whole suite)
	cargo test --locked

integrity:      ## deterministic gate: block silent test-weakening vs git (no-op without history)
	bash scripts/check-test-integrity.sh

verify:         ## the composite gate CI runs — must pass before anyone says "done"
	@$(MAKE) -s lint
	@$(MAKE) -s test
	@$(MAKE) -s integrity
	@echo "verify: PASS"

doctor:         ## is this machine ready? (toolchain, versions, required files)
	bash scripts/doctor.sh

install:        ## put `hippo-task` on your PATH (~/.cargo/bin)
	cargo install --locked --path .

demo:           ## self-running narrated demo of the real binary
	./playground/demo.sh --auto

play:           ## interactive terminal playground that drives the real binary
	./playground/play.sh

ui:             ## optional board/list UI for this project's existing task store
	cargo run --locked -- ui

playground-ui:  ## scratch-store development playground (Live/Simulate)
	python3 playground/serve.py
