.PHONY: build
build:
	cargo build --release

.PHONY: build-runtime
build-runtime:
	cargo build --release -p dbc-runtime
