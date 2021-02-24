# 如何加入DBC验证节点？

1. 编译DBC chain

   ```bash
   # 安装依赖，rust，subkey
   curl https://getsubstrate.io -sSf | bash -s -- --fast
   source ~/.cargo/env
   cargo install --force subkey --git https://github.com/paritytech/substrate --version 2.0.0 --locked
   
   # 编译dbc-chain
   git clone https://github.com/DeepBrainChain/DeepBrainChain-MainChain.git
   cd DeepBrainChain-MainChain && git checkout dbc-dev
   cargo build --release
   ```

2. 生成资金账户
   可选步骤：当您想使用别的资金账户时，可以略过这一步。

   ```bash
   # 生成stash账户 (用于存储现金)
   subkey generate --scheme sr25519

   # 以下为生成的内容：
   Secret phrase `success extra health pupil cactus find better cat layer boss renew room` is account:
     Secret seed:      0x91c96acae5f3b79682ea1db1b94f81fa1915bd2981b345b9a90f8b64786d8ffe
     Public key (hex): 0x22150e8093537cee480256fcaa2e9a2883bfea41226ecbfd168c980f42f69135
     Account ID:       0x22150e8093537cee480256fcaa2e9a2883bfea41226ecbfd168c980f42f69135
     SS58 Address:     5CqPjts5GYvR1XhwFLnFZAph4k76m3qatSAXCt1AwkFUiM6B
   ```

   **请记下生成的内容，请勿使用上面的账户。**

   ***TODO：获取一定量的`DBC` token ，以保证你的`stash账户`有一定量的DBC***

   ***TODO: 生成controller账户，并与stash账户进行绑定***

   为了账户的安全，您也可以生成一个账户(`Controller账户`)用于控制资金账户(`Stash账户`)。如果您想要这么做，再生成一个sr25519的账户作为Controller账户。在后面进行bond 操作的时候，将controller账户设置为您的controller账户。

3. 运行同步节点

   ```bash
   ./target/release/substrate \
   	--base-path ./db_data \
   	--chain ./dbcSpecRaw.json \
   	--pruning=archive \
   	--port 30333 \
   	--ws-port 9944 \
   	--rpc-port 9933 \
   	--rpc-cors=all \
   	--bootnodes /ip4/111.44.254.180/tcp/30333/p2p/12D3KooWNJRVErXu6PvFcfCCQZFBAp6oU7BPEz5vWQZrLoift6TG
   ```
   

查看同步状态：你可以根据`target`与`best`的比较来判断是否同步已经完成, 也可以通过：https://telemetry.polkadot.io/#list/DBC%20Testnet 查看当前区块块高，通过与已同步的块高比较，判断同步是否完成。

***Tips: 判断同步是否完成: 通过比较 target（目标块高）和 best（当前已同步）来判断同步进度，当target与best相差不大（如100以内）时，可以认为已经完成同步。***

![image-20210126021938613](join_dbc_testnet.assets/image-20210126021938613.png)

**参数说明：**

`--base-path`：指定该区块链存储数据的目录。如果不指定，将使用默认路径。如果目录不存在，将会为你自动创建。如果该目录已经有了区块链数据，将会报错，这时应该选择不同的目录或清除该目录内容

`--pruning=archive`：以归档的方式启动区块链

`--port`：指定你的p2p监听端口。`30333` 是默认端口，如果你想使用默认端口可以省略该参数。

`--ws-port`：指定WebSocket监听的端口。默认值是`9944`.

`--rpc-port`：指定节点监听RPC通信的端口。`9933`是默认值，这个参数可以省略。

`--rpc-cores`：指定哪些请求来源的地址能够访问该节点。值可以是逗号分割的地址(protocol://domain 或一个`null`值)，all表示禁用请求来源检查。

`--bootnodes`：指定引导节点地址。

4. 在第3步同步节点数据完成之后，关闭同步命令。然后以验证人的方式运行节点：

   ```bash
   nohup ./target/release/substrate \
   	--base-path ./db_data \
   	--chain ./dbcSpecRaw.json \
   	--validator \
   	--name YourNodeName \
   	--port 30333 \
   	--ws-port 9944 \
   	--rpc-port 9933 \
   	--rpc-cors=all \
   	--bootnodes /ip4/111.44.254.180/tcp/30333/p2p/12D3KooWNJRVErXu6PvFcfCCQZFBAp6oU7BPEz5vWQZrLoift6TG 1>dbc_node.log 2>&1 &
   ```
   

注意：这里 `--name` 是设置你节点的名称，你可以为你的节点起一个独一无二容易辨认的名称，别人将能在网络上看到它。

5. 生成`rotateKey`

   在运行验证人节点命令的机器上运行下面命令

   ```bash
   curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "author_rotateKeys", "params":[]}' http://localhost:9933
   ```

6. 登陆你的`资金账户`（通过`polkadot{.js}`浏览器插件, 导入第二步的`Secret phrase`），打开[https://test.dbcwallet.io/?rpc=wss://infotest.dbcwallet.io#/explorer ](https://test.dbcwallet.io/?rpc=wss://infotest.dbcwallet.io#/explorer)  导航到`Accounts`你将能看到你的余额：(安装`polkadot{.js}`插件：Chrome [Chrome web store](https://chrome.google.com/webstore/detail/polkadot{js}-extension/mopnmbcafieddcagagdcbnhejhlodfdd),  Firefox：[Firefox add-ons](https://addons.mozilla.org/en-US/firefox/addon/polkadot-js-extension/))

   ![image-20210121194808850](join_dbc_testnet.assets/image-20210121194808850.png)

   

   导航到`Staking > Account actions`，点击`stash`![image-20210121194953014](join_dbc_testnet.assets/image-20210121194953014.png)

   设置bond的金额（确保除了bond的数额，您的账户中还有余额以用来发送交易）：

   ![image-20210121195033167](join_dbc_testnet.assets/image-20210121195033167.png)

   **说明：**

   + `Stash account`：你的资金账户，这里我们bond 45 DBC，确保账户中余额至少有这么多

   + `controller account`：这个账户也应该有少量的DBC来发送开始和停止验证人的交易

   + `value bonded`：你想要bond/stake多少DBC, 请注意，你不需要bond账户中所有的余额，另外你随后可以增加bond的数额。

   + `payment destination`：验证人获得的奖励将会被发给这个账户。这个账户可以设置成任何账户。


7. 设置`rotateKey`:

   在执行了bond之后，您将能够在Polkadot上看到`Session Key`的按钮：

   ![image-20210121195307711](join_dbc_testnet.assets/image-20210121195307711.png)

   点击它，并将步骤5生成的`rotateKeys`填入。

   ![image-20210121200709277](join_dbc_testnet.assets/image-20210121200709277.png)

   现在，你可以到 `Telemetry` 看到你的节点了！

   ![image-20210121234945030](join_dbc_testnet.assets/image-20210121234945030.png)

9. 设置参加验证人选举

   完成了上述步骤后，你将能看到`Validate`的按钮。点击`Validate` 按钮，
   
   ![image-20210121235144583](join_dbc_testnet.assets/image-20210121235144583.png)
   
   这时你将需要设置验证人偏好。
   
   ![image-20210121235217665](join_dbc_testnet.assets/image-20210121235217665.png)
   
   在 `reward commission percentage`栏目中，你将需要输入你作为验证人的收益偏好。然后点击右下角`Validate`，并发送交易。
   
   这时，在`Waiting`界面，你将能看到你的账户正在等待下个`Era`，来参加选举成为验证人节点。
   
   ![image-20210121235451552](join_dbc_testnet.assets/image-20210121235451552.png)
   
   

## 如何领取节点奖励？

在浏览器插件polkadot中登陆你的stash账户，在 `Staking > Payouts` 中，你将能看到待领取的奖励：

![image-20210122091057746](join_dbc_testnet.assets/image-20210122091057746.png)

点击右下角的Payout，发送交易即可。
