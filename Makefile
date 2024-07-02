.PHONY: build
build:
	cargo build --release

.PHONY: build-runtime
build-runtime:
	cargo build --release -p dbc-runtime

.PHONY: build-try-runtime
build-try-runtime:
	cargo build --features try-runtime --release -p dbc-runtime

.PHONY: fmt
fmt:
	cargo fmt --all

.PHONY: run
run:
	cargo run -- --dev -lruntime=debug --ws-port=9944 --ws-external --rpc-port=8545 --rpc-external --rpc-cors=all --rpc-methods=unsafe --pruning=archive

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

.PHONY: try-runtime-snap
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
