#!/usr/bin/env bash

ws="ws://127.0.0.1:9944"
tf="../../types.json"

echo "script pid is: " $$

# https://blog.csdn.net/jinjiaoooo/article/details/38349603
# pstree pid -p| awk -F"[()]" '{print $2}'| xargs kill -9

echo "# Starting price server..."
python ../simple_server.py ../price.json 8000 1>server1.log 2>&1 &
python ../simple_server.py ../price.json 8001 1>server2.log 2>&1 &
python ../simple_server.py ../price.json 8002 1>server3.log 2>&1 &
python ../simple_server.py ../price.json 8003 1>server4.log 2>&1 &

# 设置初始价格
node tx_by_root.js --port $ws --type-file $tf \
    --module dbcPriceOcw --func addPriceUrl \
    "http://127.0.0.1:8001"

# 委员会每次抢单质押数量
node tx_by_root.js --port $ws --type-file $tf --module leaseCommittee --func setStakedUsdPerOrder \
    16000000

# 增加三个委员会
# Dave
node tx_by_root.js --port $ws --type-file $tf --module leaseCommittee --func addCommittee \
    5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy
# Eve
node tx_by_root.js --port $ws --type-file $tf --module leaseCommittee --func addCommittee \
    5HGjWAeFDfFCWPsjFQdVV2Msvz2XtMktvgocEZcCj68kUMaw
# FERDIE
node tx_by_root.js --port $ws --type-file $tf --module leaseCommittee --func addCommittee \
    5CiPPseXPECbkjWCa6MnjNokrgYjMqmKndv2rSnekmSK2DjL


# 设置每次交易的固定费率, 10DBC
node tx_by_root.js --port $ws --type-file $tf --module genericFunc --func setFixedTxFee \
    10000000000000000

# 设置单卡最多质押数量：100000 DBC
node tx_by_root.js --port $ws --type-file $tf --module onlineProfile --func setGpuStake \
    100000000000000000000

# 设置奖励开始Era时间
node tx_by_root.js --port $ws --type-file $tf --module onlineProfile --func setRewardStartEra \
    1

# 设置每个Phase中，奖励/Era
node tx_by_root.js --port $ws --type-file $tf --module onlineProfile --func setPhaseNRewardPerEra \
    0 200000000000000000000
node tx_by_root.js --port $ws --type-file $tf --module onlineProfile --func setPhaseNRewardPerEra \
    1 100000000000000000000

# 设置单卡质押价值上限 7700 USD ~~ 50000 RMB
node tx_by_root.js --port $ws --type-file $tf --module onlineProfile --func setStakeUsdLimit \
   7700000000

# 设置标准GPU的租金价格/算力点数
node tx_by_root.js --port $ws --type-file $tf --module onlineProfile --func setStandardGpuPointPrice \
    1000 1000

wait
