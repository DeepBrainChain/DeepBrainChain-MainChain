# query avg block time

currentHeight=767000

for i in {0..10}
do
    let queryHeight=currentHeight-2880*7*i
    echo "queryHeight: " $queryHeight
    node query_storage.js --port wss://info.dbcwallet.io --type-file ../../dbc_types.json --rpc-file ../../dbc_rpc.json --module timestamp --func now --at-height $queryHeight
done
