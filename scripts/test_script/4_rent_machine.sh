#!/usr/bin/env bash

# 设置租用合约
node tx_by_root.js --port $ws --type-file $tf --rpc-file $rpc --module rentMachine --func setRentPot $pot_stash

# 发送租用请求
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module rentMachine --func rentMachine \
    --key $renter_key $machine 100

# 发送确认租用
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module rentMachine --func confirmRent \
    --key $renter_key $machine

# 续租
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module rentMachine --func addRent \
    --key $renter_key $machine 100
