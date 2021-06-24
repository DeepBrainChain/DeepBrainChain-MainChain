#!/usr/bin/env bash

# stash账户设置控制账户.控制账户为：controller; 该机器ID为machine; 机器stash账户为stash:
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func setController \
    --key $stash_key $controller

# FIXME: 应该补充信息
# 绑定机器: controller为控制人，绑定了一个机器：machine, 受益账户为stash
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func bondMachine \
    --key $controller_key $machine

# 生成签名信息，由机器签名
node gen_signature.js --key $machine_key --msg $machine$stash
### MSG: 5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty5HpG9w8EBLe5XCrbczpwq5TSXvedjrBGCwqxK1iQ7qUsSWFc
### SignedBy: 5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty
### Signature: 0x0089673806c55e6e9d4ce4ea46c6d24736599c0f48f16fa7719b303b6a204602fcf33946d3a588a87f9619db0890b027d1ad358fa9a3de10f57e03e2a3423782

# 由控制人提交，机器地址提交签名，与资金账户绑定
sig="0x0089673806c55e6e9d4ce4ea46c6d24736599c0f48f16fa7719b303b6a204602fcf33946d3a588a87f9619db0890b027d1ad358fa9a3de10f57e03e2a3423782"
node tx_by_user.js --port $ws --type-file $tf --rpc-file $rpc --module onlineProfile --func machineSetStash \
    --key $controller_key --sig $sig $machine$stash
