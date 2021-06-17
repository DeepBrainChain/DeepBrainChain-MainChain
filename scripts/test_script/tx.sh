node tx_by_root.js \
    --port "ws://127.0.0.1:9944" \
    --type-file "../../types.json" \
    --module dbcPriceOcw \
    --func addPriceUrl \
    "http://127.0.0.1:8001"
