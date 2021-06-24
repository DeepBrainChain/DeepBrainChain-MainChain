#!/usr/bin/env bash

# 设置DBC价格
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module dbcPriceOcw --func addPriceUrl "http://127.0.0.1:8000"
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module dbcPriceOcw --func addPriceUrl "http://127.0.0.1:8001"
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module dbcPriceOcw --func addPriceUrl "http://127.0.0.1:8002"
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module dbcPriceOcw --func addPriceUrl "http://127.0.0.1:8003"

# 委员会每次抢单质押数量
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func setStakedUsdPerOrder 20000000
# 设置每次交易的固定费率, 10DBC
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module genericFunc --func setFixedTxFee 10
# 设置单卡最多质押数量：100000 DBC
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func setGpuStake 100000 # 设置奖励开始Era时间
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func setRewardStartEra 0
# 设置每个Phase中，奖励/Era
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func setPhaseNRewardPerEra 0 1000000
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func setPhaseNRewardPerEra 1 1000000
# 设置单卡质押价值上限 7700 USD ~~ 50000 RMB
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func setStakeUsdLimit 7700000000

# TODO: 传递一个结构体作为参数
# 设置标准GPU的租金价格/算力点数 FIXME: 应该传第一个参数
# node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func setStandardGpuPointPrice 1000 200000000

# 增加三个委员会
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func addCommittee \
    $committee_1
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func addCommittee \
    $committee_2
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func addCommittee \
    $committee_3

node gen_boxpubkey.js --key $committee_1_key # NOTE: committee_1_boxpubkey
node gen_boxpubkey.js --key $committee_2_key # NOTE: committee_2_boxpubkey
node gen_boxpubkey.js --key $committee_3_key # NOTE: committee_3_boxpubkey

# 提交pubkey
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func committeeSetBoxPubkey \
    --key $committee_1_key $committee_1_boxpubkey
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func committeeSetBoxPubkey \
    --key $committee_2_key $committee_2_boxpubkey
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module committee --func committeeSetBoxPubkey \
    --key $committee_3_key $committee_3_boxpubkey
