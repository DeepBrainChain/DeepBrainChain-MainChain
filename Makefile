.PHONY: build
build:
	cargo build --release

.PHONY: build-runtime
build-runtime:
	cargo build --release -p dbc-runtime

.PHONY: run
run:
	cargo run --features dev-mode -- --dev -lruntime=debug --ws-port=9944 --ws-external --rpc-port=8545 --rpc-external --rpc-cors=all --rpc-methods=unsafe --pruning=archive
