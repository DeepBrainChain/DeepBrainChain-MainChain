#!/usr/bin/env bash

# 设置环境变量...
ws="ws://127.0.0.1:9944"
tf="../../dbc_types.json"
rpc="../../dbc_rpc.json"

echo "### Starting price server..."
python ../simple_server.py ../price.json 8000 1>server1.log 2>&1 &
python ../simple_server.py ../price.json 8001 1>server2.log 2>&1 &
python ../simple_server.py ../price.json 8002 1>server3.log 2>&1 &
python ../simple_server.py ../price.json 8003 1>server4.log 2>&1 &

# 机器租用者
renter="5EeJ5NBZbsQwr1ixDkrhEvsQtdtJ9bWtvRuvGP4LqLRgv8SU"
renter_key="0x531738dd107d421f7e44e9c0d49ec1fc5ebec2a01d7e8bf88004fe6511478e49"

machine="5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty"
machine_key="0x398f0c28f98885e046333d4a41c19cee4c37368a9832c6502f6cfd182e2aef89"

controller="5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy"
controller_key="0x868020ae0687dda7d57565093a69090211449845a7e11453612800b663307246"

stash="5HpG9w8EBLe5XCrbczpwq5TSXvedjrBGCwqxK1iQ7qUsSWFc"
stash_key="0x1a7d114100653850c65edecda8a9b2b4dd65d900edef8e70b1a6ecdcda967056"

eve_key="0x786ad0e2df456fe43dd1f91ebca22e235bc162e0bb8d53c633e8c85b2af68b7a"
ferdie_key="0x42438b7883391c05512a938e36c2df0131e088b3756d6aa7a755fbff19d2f842"

# 租金发放地址
pot_stash="5C7Qu33GDq2FueQFLo6c2uooHRjrj6QrHcoxyQPFSRukAkQY"
# 0x41141144ba79b58f6a98b49413d68b3971aaad28fe04adf7946b8a8c76e20237

committee_1=""
committee_2=""
committee_3=""
committee_1_key=""
committee_2_key=""
committee_3_key=""

# echo "script pid is: " $$
# pstree pid -p| awk -F"[()]" '{print $2}'| xargs kill -9

# 查询存储：当前平均价格
node query_storage.js --port $ws --type-file $tf --rpc-file $rpc --module dbcPriceOcw --func avgPrice
node query_storage.js --port $ws --type-file $tf --rpc-file $rpc --module dbcPriceOcw --func priceURL


# 查询存储：
node query_storage.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func committeeStakeDBCPerOrder
node query_storage.js --port $ws --type-file $tf --rpc-file $rpc --module genericFunc --func fixedTxFee
node query_storage.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func stakePerGPU
