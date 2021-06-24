#!/usr/bin/env bash

# 提交机器信息Hash
# python ../hash_str.py
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module leaseCommittee --func submitConfirmHash \
    --key $committee_1_key --hash "0x6e10845ba0abcc5e058d0ed395d34a98" $machine

# 提交原始信息

# 查询派单
# TODO: 通过js查询数据
