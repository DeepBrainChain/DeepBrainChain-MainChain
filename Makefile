.PHONY: build
build:
	cargo build --release

.PHONY: build-runtime
build-runtime:
	cargo build --release -p dbc-runtime

.PHONY: fmt-check
check-fmt:
	cargo +nightly fmt --all -- --check

.PHONY: fmt
fmt:
	cargo +nightly fmt --all

.PHONY: check-try-runtime
check-try-runtime:
	cargo check --features try-runtime

.PHONY: try-runtime
try-runtime:
	cargo build --features try-runtime --release
	./target/release/dbc-chain try-runtime --runtime ./target/release/wbuild/dbc-runtime/dbc_runtime.compact.compressed.wasm --chain= on-runtime-upgrade live --uri wss://info1.dbcwallet.io
