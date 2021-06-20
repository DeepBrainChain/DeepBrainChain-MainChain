#!/usr/bin/env bash

ws="ws://127.0.0.1:9944"
tf="../../dbc_types.json"
rpc="../../dbc_rpc.json"

# echo "script pid is: " $$
# https://blog.csdn.net/jinjiaoooo/article/details/38349603
# pstree pid -p| awk -F"[()]" '{print $2}'| xargs kill -9

# echo "# Starting price server..."
python ../simple_server.py ../price.json 8000 1>server1.log 2>&1 &
python ../simple_server.py ../price.json 8001 1>server2.log 2>&1 &
python ../simple_server.py ../price.json 8002 1>server3.log 2>&1 &
python ../simple_server.py ../price.json 8003 1>server4.log 2>&1 &

# 设置DBC价格
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module dbcPriceOcw --func addPriceUrl \
    "http://127.0.0.1:8000"
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module dbcPriceOcw --func addPriceUrl \
    "http://127.0.0.1:8001"
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module dbcPriceOcw --func addPriceUrl \
    "http://127.0.0.1:8002"
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module dbcPriceOcw --func addPriceUrl \
    "http://127.0.0.1:8003"

# 委员会每次抢单质押数量
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func setStakedUsdPerOrder \
    16000000

# 设置每次交易的固定费率, 10DBC
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module genericFunc --func setFixedTxFee \
    10000

# 设置单卡最多质押数量：100000 DBC
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func setGpuStake \
    10000

# stash账户设置控制账户
bob_stash_key="0x1a7d114100653850c65edecda8a9b2b4dd65d900edef8e70b1a6ecdcda967056"
# 控制账户为：Dave
dave="5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy"
# 该机器ID为Bob
bob="5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty"
# 机器stash账户为BobStash:
bob_stash="5HpG9w8EBLe5XCrbczpwq5TSXvedjrBGCwqxK1iQ7qUsSWFc"
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func setController \
    --key $bob_stash_key $dave

# 绑定机器

# 私钥
bob_key="0x398f0c28f98885e046333d4a41c19cee4c37368a9832c6502f6cfd182e2aef89"
eve_key="0x786ad0e2df456fe43dd1f91ebca22e235bc162e0bb8d53c633e8c85b2af68b7a"
ferdie_key="0x42438b7883391c05512a938e36c2df0131e088b3756d6aa7a755fbff19d2f842"
dave_key="0x868020ae0687dda7d57565093a69090211449845a7e11453612800b663307246"

# dave为控制人，绑定了一个机器：Bob, 受益账户为BobStash
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func bondMachine \
    --key $dave_key $bob

# 生成签名信息，由机器签名
node gen_signature.js --key $bob_key --msg $bob$bob_stash
### MSG: 5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty5HpG9w8EBLe5XCrbczpwq5TSXvedjrBGCwqxK1iQ7qUsSWFc
### SignedBy: 5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty
### Signature: 0x0089673806c55e6e9d4ce4ea46c6d24736599c0f48f16fa7719b303b6a204602fcf33946d3a588a87f9619db0890b027d1ad358fa9a3de10f57e03e2a3423782


# 由控制人提交，机器地址提交签名，与资金账户绑定
sig="0x0089673806c55e6e9d4ce4ea46c6d24736599c0f48f16fa7719b303b6a204602fcf33946d3a588a87f9619db0890b027d1ad358fa9a3de10f57e03e2a3423782"
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func machineSetStash \
    --key $dave_key --sig $sig $bob $bob_stash


# 增加三个委员会
# Dave
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func addCommittee \
    5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy
# Eve
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func addCommittee \
    5HGjWAeFDfFCWPsjFQdVV2Msvz2XtMktvgocEZcCj68kUMaw
# FERDIE
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func addCommittee \
    5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL

# 三个委员会添加Pubkey
# 首先生成Pubkey，如Dave: node gen_boxpubkey.js --key "0x868020ae0687dda7d57565093a69090211449845a7e11453612800b663307246"
# Dave: 0xa7804e30caa5645e97489b2d4711e3d8f4e17a683338cba97a53b960648f0438
# Eve: 0x5eec53877f4b18c8b003fa983d27ef2e5518b7e4d08d482922a7787f2ea75529
# Ferdie: 0xf660309770b2bd379e2514d88c146a7ddc3759533cf06d9fb4b41159e560325e

# 提交pubkey
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func committeeSetBoxPubkey \
    --key $dave_key 0xa7804e30caa5645e97489b2d4711e3d8f4e17a683338cba97a53b960648f0438
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func committeeSetBoxPubkey \
    --key $eve_key 0x5eec53877f4b18c8b003fa983d27ef2e5518b7e4d08d482922a7787f2ea75529
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func committeeSetBoxPubkey \
    --key $ferdie_key 0xf660309770b2bd379e2514d88c146a7ddc3759533cf06d9fb4b41159e560325e


# 查询派单
# TODO: 通过js查询数据


# 设置奖励开始Era时间
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func setRewardStartEra \
    1

# 设置每个Phase中，奖励/Era
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func setPhaseNRewardPerEra \
    0 10000
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func setPhaseNRewardPerEra \
    1 10000

# 设置单卡质押价值上限 7700 USD ~~ 50000 RMB
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func setStakeUsdLimit \
   10000

# 设置标准GPU的租金价格/算力点数
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func setStandardGpuPointPrice \
    1000 1000

