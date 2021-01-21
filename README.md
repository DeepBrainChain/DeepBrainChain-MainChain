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



## 配置

### 奖励数量：

[0～3)年：每年10^9 DBC

[3~8)年: 每年 0.5 * 10^9 DBC

[8, 8+)年：每年0.5 * [total_DBC_from_ + 2.5*10^9] / 5 DBC

### 奖励规则：

+ 出块时间：6 seconds
+ epoch duration： 1 hours
+ era duartion: 6 hours (一个选举周期，也是奖励计算周期)
  + 每个区块生成后，记录区块生产者的`ErasRewardPoints`，`Reward Points`增加规则
    + 主链区块生产者增加20点reward
    + 叔区块生产者增加2点reward
    + 引用叔区块的生产者增加1点reward
+ 奖励保留时间：**84 era (21天)**



+ 验证者（即出块节点）奖励 = 总奖励 * 自定义比例的分佣 + 生于部分的奖励 * 验证者stake占节点的比例

+ 能获得奖励的提名者数量：128名 （按stake数量排名），奖励 = （总奖励 - 验证者自定义比例分佣 ）* stake占节点总stake比例
+ 验证人在相同的工作中获得相同数量的奖励

### 节点设置

+ **设置phase0, phase1, phase2每年奖励数目：**

  `Developer`=>`Sudo access` =>( `staking/setPhase0Reward`, `staking/setPhase1Reward`, `staking/setPhase2Reward`)

+ 设置验证人数量（root）：`Developer`=>`Sudo access`=>`Contracts`=>`staking/set_validator_count`

+ 增加验证人数量 （root）: `staking/in`

+ 领取配置: `Network` => `Staking` => `Payouts`



## 如何成为验证人节点

[如何加入dbc验证节点](docs/join_dbc_testnet.md)