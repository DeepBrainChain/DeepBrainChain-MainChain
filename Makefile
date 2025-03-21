# use `cargo nextest run` if cargo-nextest is installed
cargo_test = $(shell which cargo-nextest >/dev/null && echo "cargo nextest run" || echo "cargo test")

.PHONY: test
test:
	${cargo_test} --all

.PHONY: build
build:
	cargo build --release

.PHONY: build-runtime
build-runtime:
	cargo build --release -p dbc-runtime

.PHONY: build-evm-tracing-runtime
build-evm-tracing-runtime:
	cargo build --features evm-tracing --release -p dbc-runtime

.PHONY: build-try-runtime
build-try-runtime:
	cargo build --features try-runtime --release -p dbc-runtime

.PHONY: fmt
fmt:
	cargo +nightly-2023-09-20 fmt --all

.PHONY: run
run:
	cargo run \
		--features evm-tracing \
		-- \
		--dev \
		-lruntime=debug,evm=trace \
		--rpc-port=9944 \
		--rpc-external \
		--rpc-cors=all \
		--rpc-methods=unsafe \
		--pruning=archive \
		--ethapi=debug,trace,txpool
		#--wasm-runtime-overrides=./runtime-overrides

NODE_URI ?=wss://info1.dbcwallet.io:443
BLOCK_HASH ?=0xc4d4e9b1a2b8c44d7859a6004c43ad6eebb61c57b6173a53fe794a6aa479a49b
.PHONY: try-runtime-live
try-runtime-live: build-try-runtime
	cargo run --features try-runtime -- \
		try-runtime \
		--runtime ./target/release/wbuild/dbc-runtime/dbc_runtime.compact.compressed.wasm \
		--chain=mainnet \
		on-runtime-upgrade \
		live \
		--uri ${NODE_URI} \
		--at ${BLOCK_HASH}

.PHONY: try-runtime-create-snap
try-runtime-create-snap:
	cargo run --features try-runtime -- \
		try-runtime \
		--runtime existing \
		create-snapshot \
		--uri ${NODE_URI} \
		--at ${BLOCK_HASH} \
		dbc-${BLOCK_HASH}.snap

.PHONY: try-runtime-upgrade-snap
try-runtime-upgrade-snap: build-try-runtime
	cargo run --features try-runtime -- \
		try-runtime \
		--runtime ./target/release/wbuild/dbc-runtime/dbc_runtime.compact.compressed.wasm \
		--chain=mainnet \
		on-runtime-upgrade \
		snap \
		-s dbc-${BLOCK_HASH}.snap
