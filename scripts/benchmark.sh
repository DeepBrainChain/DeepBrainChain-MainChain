# ./target/release/dbc-chain benchmark \
#     --chain dev \                  # Configurable Chain Spec
#     --execution=wasm \             # Always test with Wasm
#     --wasm-execution=compiled \    # Always used `wasm-time`
#     --pallet committee \     # Select the pallet
#     --extrinsic add_committee \         # Select the extrinsic
#     --steps 10 \                   # Number of samples across component ranges
#     --repeat 20 \                  # Number of times we repeat a benchmark
#     --output ./a.txt \              # Output benchmark results into a folder or file

./target/release/dbc-chain benchmark \
    --chain dev \
    --execution=wasm \
    --pallet committee \
    --extrinsic add_committee \
    --steps 10 \
    --repeat 20 --output ./pallets/committee/src/weights.rs \
    --template=./scripts/frame-weight-template.hbs
    # --wasm-execution=compiled \
