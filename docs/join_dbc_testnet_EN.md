# How to run a DBC validator?

1. Compile DBC chain

   ```bash
   # install rust，subkey
   curl https://getsubstrate.io -sSf | bash -s -- --fast
   source ~/.cargo/env
   cargo install --force subkey --git https://github.com/paritytech/substrate --version 2.0.0 --locked
   
   # compile dbc-chain
   git clone https://github.com/DeepBrainChain/DeepBrainChain-MainChain.git
   cd DeepBrainChain-MainChain && git checkout dbc-dev
   cargo build --release
   ```

2. Generate stash account

   ```bash
   subkey generate --scheme sr25519
   
   # The following is generated：
   Secret phrase `success extra health pupil cactus find better cat layer boss renew room` is account:
     Secret seed:      0x91c96acae5f3b79682ea1db1b94f81fa1915bd2981b345b9a90f8b64786d8ffe
     Public key (hex): 0x22150e8093537cee480256fcaa2e9a2883bfea41226ecbfd168c980f42f69135
     Account ID:       0x22150e8093537cee480256fcaa2e9a2883bfea41226ecbfd168c980f42f69135
     SS58 Address:     5CqPjts5GYvR1XhwFLnFZAph4k76m3qatSAXCt1AwkFUiM6B
   ```

   **Please record the output of the command, and don't use the account generated above**

   ***TODO：Get some `DBC` token ，ensure your`stash account` have some DBC***

   ***TODO: generate a controller account，and bond with your stash account***

   For your account security, you can also generate an account (`Controller account`) to control the `stash account`. If you want to do this, create a sr25519 account as the `Controller account`. When performing the bond operation in setp 6, use the controller account  in controller account box. 

3. Synchronize Chain Data

   ```bash
   ./target/release/substrate \
   	--base-path ./account5 \
   	--chain ./dbcSpecRaw.json \
   	--pruning=archive \
   	--port 30333 \
   	--ws-port 9944 \
   	--rpc-port 9933 \
   	--rpc-cors=all \
   	--bootnodes /ip4/111.44.254.180/tcp/30333/p2p/12D3KooWNJRVErXu6PvFcfCCQZFBAp6oU7BPEz5vWQZrLoift6TG
   ```

   If you are interested in determining how much longer you have to go, your server logs (printed to STDOUT from the `substrate` process) will tell you the latest block your node has processed and verified. You can then compare that to the current highest block via [Telemetry](https://telemetry.polkadot.io/#list/DBC%20Testnet CC1) or the [PolkadotJS Block Explorer](https://test.dbcwallet.io/?rpc=wss://infotest.dbcwallet.io#/explorer).

   **Flags in detail：**

   `--base-path`：Specifies a directory where Substrate should store all the data  related to this chain. If this value is not specified, a default path  will be used. If the directory does not exist it will be created for  you. If other blockchain data already exists there you will get an  error. Either clear the directory or choose a different one.

   `--chain ./dbcSpecRaw.json`：Specifies which chain specification to use. 

   `--pruning=archive`：synchronize chain data

   `--port`：Specifies the port that your node will listen for p2p traffic on. `30333` is the default and this flag can be omitted if you're happy with the  default. 

   `--ws-port`：Specifies the port that your node will listen for incoming WebSocket traffic on. The default value is `9944`. 

   `--rpc-port`：Specifies the port that your node will listen for incoming RPC traffic on. `9933` is the default, so this parameter may be omitted.

   `--rpc-cores`：Specify browser Origins allowed to access the HTTP & WS RPC servers.

   `--bootnodes`：specified a single boot node.

   *** Tips: how to figure out if synchronize is finished: by compare target（target block height）和best（current block height）to know how much has Synced: ***

![image-20210126021938613](join_dbc_testnet.assets/image-20210126021938613.png)

4. After synchronizing the node data finished, kill the program. Then run the node as a validator: 

   ```bash
   ./target/release/substrate \
   	--base-path ./account5 \
   	--chain ./dbcSpecRaw.json \
   	--validator \
   	--name MyNode5 \
   	--port 30333 \
   	--ws-port 9944 \
   	--rpc-port 9933 \
   	--rpc-cors=all \
   	--bootnodes /ip4/111.44.254.180/tcp/30333/p2p/12D3KooWNJRVErXu6PvFcfCCQZFBAp6oU7BPEz5vWQZrLoift6TG
   ```

   You can give your validator any name that you like, but note that others will be able to see it, and it will be included in the list of all servers using the same telemetry server. Since numerous people are using telemetry, it is recommended that you choose something likely to be unique.

   You may want to run this command backend, just add `nohup` before this command and `&` after this command.

5. generate`rotateKey`

   Run the following command in the terminal, and record the result.

   ```bash
   curl -H "Content-Type: application/json" -d '{"id":1, "jsonrpc":"2.0", "method": "author_rotateKeys", "params":[]}' http://localhost:9933
   ```

6. Login your`stash account`（login by`polkadot{.js}` browser adds-on），open [https://test.dbcwallet.io/?rpc=wss://infotest.dbcwallet.io#/explorer ](https://test.dbcwallet.io/?rpc=wss://infotest.dbcwallet.io#/explorer)  and navigate to`Accounts`, you will see your balance.

   (install`polkadot{.js}`adds-on：Chrome [Chrome web store](https://chrome.google.com/webstore/detail/polkadot{js}-extension/mopnmbcafieddcagagdcbnhejhlodfdd), Firefox：[Firefox add-ons](https://addons.mozilla.org/en-US/firefox/addon/polkadot-js-extension/))

   ![image-20210121194808850](join_dbc_testnet.assets/image-20210121194808850.png)

   

   navigate to`Staking > Account actions`，click `stash`![image-20210121194953014](join_dbc_testnet.assets/image-20210121194953014.png)

   you should set bond balance（Make sure not to bond all your DBC balance since you will be unable to pay transaction fees from your bonded balance.）：

   ![image-20210121195033167](join_dbc_testnet.assets/image-20210121195033167.png)

   **Description：**

   + `Stash account`：Select your Stash account. In this example, we will bond 45 DBC - make sure that your Stash account contains *at least* this much. You can, of course, stake more than this.
   + `controller account`：Select the Controller account created earlier. This account will also need a small amount of DBC in order to start and stop validating.
   + `value bonded`：How much DBC from the Stash account you want to bond/stake. Note that you do not need to bond all of the DBC in that account. Also note that you can always bond *more* DBC later.
   + `payment destination`：The account where the rewards from validating are sent. 


7. Set Session Keys

   After bond your stash，you can see in Polkadot UI `Session Key`button：

   ![image-20210121195307711](join_dbc_testnet.assets/image-20210121195307711.png)

   click it and input the result in step 5`rotateKeys`。

   ![image-20210121200709277](join_dbc_testnet.assets/image-20210121200709277.png)

   Now, check in `Telemetry` and you can see your node.![image-20210121234945030](join_dbc_testnet.assets/image-20210121234945030.png)

9. Validate

   After steps above you can see`Validate` button and click it,
   
   ![image-20210121235144583](join_dbc_testnet.assets/image-20210121235144583.png)
   
   you should set your preference as a validator.![image-20210121235217665](join_dbc_testnet.assets/image-20210121235217665.png)
   
   in  `reward commission percentage` you should input your commission preference. Then clieck `Validate` to send this transaction。
   
   Then，in `Waiting` page，you are waiting status`Era` to be a validator.
   
   ![image-20210121235451552](join_dbc_testnet.assets/image-20210121235451552.png)
   
   

## How to get your rewards payout？

Login in your `polkadot{.js}` account，navigate to `Staking > Payouts` ，you may see the rewards to be payout：

![image-20210122091057746](join_dbc_testnet.assets/image-20210122091057746.png)

chick `Payout` button to send a transaction.
