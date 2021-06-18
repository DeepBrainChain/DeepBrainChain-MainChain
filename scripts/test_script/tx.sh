#!/usr/bin/env bash

ws="ws://127.0.0.1:9944"
tf="../../dbc_types.json"

# echo "script pid is: " $$
# https://blog.csdn.net/jinjiaoooo/article/details/38349603
# pstree pid -p| awk -F"[()]" '{print $2}'| xargs kill -9

# echo "# Starting price server..."
python ../simple_server.py ../price.json 8000 1>server1.log 2>&1 &
python ../simple_server.py ../price.json 8001 1>server2.log 2>&1 &
python ../simple_server.py ../price.json 8002 1>server3.log 2>&1 &
python ../simple_server.py ../price.json 8003 1>server4.log 2>&1 &

# 设置DBC价格
node tx_by_root.js --port $ws --type-file $tf --module dbcPriceOcw --func addPriceUrl \
    "http://127.0.0.1:8000"
node tx_by_root.js --port $ws --type-file $tf --module dbcPriceOcw --func addPriceUrl \
    "http://127.0.0.1:8001"
node tx_by_root.js --port $ws --type-file $tf --module dbcPriceOcw --func addPriceUrl \
    "http://127.0.0.1:8002"
node tx_by_root.js --port $ws --type-file $tf --module dbcPriceOcw --func addPriceUrl \
    "http://127.0.0.1:8003"

# 委员会每次抢单质押数量
node tx_by_root.js --port $ws --type-file $tf --module committee --func setStakedUsdPerOrder \
    16000000

# 设置每次交易的固定费率, 10DBC
node tx_by_root.js --port $ws --type-file $tf --module genericFunc --func setFixedTxFee \
    10000

# 设置单卡最多质押数量：100000 DBC
node tx_by_root.js --port $ws --type-file $tf --module onlineProfile --func setGpuStake \
    10000

# stash账户设置控制账户
bob_stash="0x1a7d114100653850c65edecda8a9b2b4dd65d900edef8e70b1a6ecdcda967056"
node tx_by_user.js --port $ws --type-file $tf --module onlineProfile --func setController \
    --key $bob_stash 5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy

# 绑定机器
# 控制账户为：Dave  5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy
# 该机器ID为Bob， 5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty
# 机器stash账户为BobStash: 5HpG9w8EBLe5XCrbczpwq5TSXvedjrBGCwqxK1iQ7qUsSWFc

# 私钥
# Bob stash: 0x1a7d114100653850c65edecda8a9b2b4dd65d900edef8e70b1a6ecdcda967056
# Bob：0x398f0c28f98885e046333d4a41c19cee4c37368a9832c6502f6cfd182e2aef89
eve_key="0x786ad0e2df456fe43dd1f91ebca22e235bc162e0bb8d53c633e8c85b2af68b7a"
ferdie_key="0x42438b7883391c05512a938e36c2df0131e088b3756d6aa7a755fbff19d2f842"
dave_key="0x868020ae0687dda7d57565093a69090211449845a7e11453612800b663307246" # Dave私钥
node tx_by_user.js --port $ws --type-file $tf --module onlineProfile --func bondMachine \
    --key $dave_key 5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty


# 增加三个委员会
# Dave
node tx_by_root.js --port $ws --type-file $tf --module committee --func addCommittee \
    5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy
# Eve
node tx_by_root.js --port $ws --type-file $tf --module committee --func addCommittee \
    5HGjWAeFDfFCWPsjFQdVV2Msvz2XtMktvgocEZcCj68kUMaw
# FERDIE
node tx_by_root.js --port $ws --type-file $tf --module committee --func addCommittee \
    5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL

# 三个委员会添加Pubkey
# 首先生成Pubkey，如Dave: node gen_boxpubkey.js --key "0x868020ae0687dda7d57565093a69090211449845a7e11453612800b663307246"
# Dave: 0xa7804e30caa5645e97489b2d4711e3d8f4e17a683338cba97a53b960648f0438
# Eve: 0x5eec53877f4b18c8b003fa983d27ef2e5518b7e4d08d482922a7787f2ea75529
# Ferdie: 0xf660309770b2bd379e2514d88c146a7ddc3759533cf06d9fb4b41159e560325e

# 提交pubkey
node tx_by_user.js --port $ws --type-file $tf --module committee --func committeeSetBoxPubkey \
    --key $dave_key  0xa7804e30caa5645e97489b2d4711e3d8f4e17a683338cba97a53b960648f0438
node tx_by_user.js --port $ws --type-file $tf --module committee --func committeeSetBoxPubkey \
    --key $eve_key  0x5eec53877f4b18c8b003fa983d27ef2e5518b7e4d08d482922a7787f2ea75529
node tx_by_user.js --port $ws --type-file $tf --module committee --func committeeSetBoxPubkey \
    --key $ferdie_key  0xf660309770b2bd379e2514d88c146a7ddc3759533cf06d9fb4b41159e560325e



# 查询派单





# 设置奖励开始Era时间
node tx_by_root.js --port $ws --type-file $tf --module onlineProfile --func setRewardStartEra \
    1

# 设置每个Phase中，奖励/Era
node tx_by_root.js --port $ws --type-file $tf --module onlineProfile --func setPhaseNRewardPerEra \
    0 10000
node tx_by_root.js --port $ws --type-file $tf --module onlineProfile --func setPhaseNRewardPerEra \
    1 10000

# 设置单卡质押价值上限 7700 USD ~~ 50000 RMB
node tx_by_root.js --port $ws --type-file $tf --module onlineProfile --func setStakeUsdLimit \
   10000

# 设置标准GPU的租金价格/算力点数
node tx_by_root.js --port $ws --type-file $tf --module onlineProfile --func setStandardGpuPointPrice \
    1000 1000

