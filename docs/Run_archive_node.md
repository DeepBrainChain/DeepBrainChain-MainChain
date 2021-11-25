# 如何运行同步节点？

## 1. 配置环境

```bash
# 安装依赖
curl https://getsubstrate.io -sSf | bash -s -- --fast
source ~/.cargo/env

# 编译dbc-chain
git clone https://github.com/DeepBrainChain/DeepBrainChain-MainChain.git
cd DeepBrainChain-MainChain
cargo build --release
```

## 2. 运行同步节点

```bash
./target/release/dbc-chain --base-path ./db_data --chain ./dbcSpecRaw.json --pruning archive --bootnodes /ip4/47.74.88.41/tcp/8947/p2p/12D3KooWD87i4TKA68P7zpGNXxUaHgvnimbgihEzDyJrmG3iGJPw
```

> 端口参数：
>
> ```
> --rpc-port 9933 #  指定你的节点监听RPC的端口。 9933 是默认值，因此该参数也可忽略
> --ws-port 9945 # 指定你的节点用于监听 WebSocket 的端口。 默认端口为 9944
> --port 30333 # 指定你用于监听 p2p 流量的节点端口。 30333 是默认端口，若无需更改，可以忽略该 flag
> ```
