#!/bin/bash

# # Assume run in docker
# docker pull ubuntu:18.04
# docker run -it --name ubuntu-test ubuntu:18.04
# apt update && apt upgrade -y && apt install curl git -y

# red='\e[91m'
# green='\e[92m'
# yellow='\e[93m'
# magenta='\e[95m'
# cyan='\e[96m'
# none='\e[0m'

# # install subkey
# curl https://getsubstrate.io -sSf | bash -s -- --fast
# source ~/.cargo/env

# # China mirror of rust crates
# echo "[source.crates-io]
# registry = \"https://github.com/rust-lang/crates.io-index\"
# replace-with = 'rustcc'
# [source.rustcc]
# registry=\"git://crates.rustcc.com/crates.io-index\"" >> ~/.cargo/config

# # install subkey to generate new accounts
# cargo install --force subkey --git https://github.com/paritytech/substrate --version 2.0.0 --locked

# # compile dbc-chain
# git clone https://github.com/DeepBrainChain/DeepBrainChain-MainChain.git
# cd DeepBrainChain-MainChain && git checkout dbc-dev
# cargo build --release

# Generate stash account
echo "Please record the following output..."

echo "Generate new account as stash account?"

generate_stash_account() {
    echo
    while :; do
        read -p "是否创建新的资金账户？[Y/N]: " create_new_stash_acct
        case $create_new_stash_acct in
            Y | y) 
                subkey generate --scheme sr25519 > stash.key
                break
                ;;
            N | n)
                break
                ;;
            *)
                echo -e "请输入 [Y/N]"
                continue
                ;;
        esac
    done
}

run_sync_node() {
    echo "运行节点同步..."
    while :; do
        read -p "请输入区块保存路径：[回车选择默认路径：./db_data]: " db_path
        [ -z "$db_path" ] && db_path="./db_data"

        echo "请勿关闭程序，等待同步完成..."

        ./target/release/substrate \
            --base-path $db_path \
            --pruning=archive \
            --port 30333 \
            --ws-port 9944 \
            --rpc-port 9933 \
            --rpc-cors=all \
            --bootnodes /ip4/111.44.254.180/tcp/30333/p2p/12D3KooWNJRVErXu6PvFcfCCQZFBAp6oU7BPEz5vWQZrLoift6TG |& \
            awk '{if ($4=="Syncing") print $7, $14; fflush()}' | awk -F "#" '{if ($2 - $3 < 10) echo "输入 CTRL + C 取消同步"}'

        break
    done
}

run_validator_node() {
    echo "运行验证人节点..."

    while :; do
        read -p  "请输入你的节点名字，别人将能从网络中看到它：" node_name
        if ![[ -z "$node_name" ]]; then
           continue
        fi
    
        nohup ./target/release/substrate \
        --base-path ./db_data \
        --validator \
        --name $node_name \
        --port 30333 \
        --ws-port 9944 \
        --rpc-port 9933 \
        --rpc-cors=all \
        --bootnodes /ip4/111.44.254.180/tcp/30333/p2p/12D3KooWNJRVErXu6PvFcfCCQZFBAp6oU7BPEz5vWQZrLoift6TG 1>node.log 2>&1 &

        break
    done
}

generate_rotateKey() {
    rotateKey = `curl http://localhost:9933 -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "author_rotateKeys", "params":[]}' 2>/dev/null |sed 's/"/ /g' | awk '{print $8}'`
}

binding_dbc() {
    echo "你应该绑定DBC，请参考..."
}

set_rotateKey() {
    
}

generate_stash_account

run_sync_node

run_validator_node

generate_rotateKey

echo "Good!"
echo $db_path
