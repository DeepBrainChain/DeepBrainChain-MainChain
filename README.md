# Substrate


## 环境配置
### 安装 rust

``` sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 安装 WASM

``` sh
rustup target add wasm32-unknown-unknown --toolchain nightly
```

### 安装 dbc-substrate

``` sh
git clone https://github.com/DeepBrainChain/DeepBrainChain-MainChain.git
```

## 编译

``` sh
cd DeepBrainChain-MainChain/ && cargo build --release
```

## 运行
### 清空区块链存储 (每次启动链之前都要执行此操作)

``` sh
cd DeepBrainChain-MainChain/ && ./target/release/substrate purge-chain --dev -y
```


### 启动区块链

``` sh
cd DeepBrainChain-MainChain/ && ./target/release/substrate --dev
```

## 打开前端页面
在浏览器里输入 https://test.dbcwallet.io
