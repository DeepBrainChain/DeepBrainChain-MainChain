import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import fs from "fs";
import minimist from "minimist";
import { createTestKeyring } from "@polkadot/keyring/testing";

async function main() {
  // 读取参数
  const args = minimist(process.argv.slice(2));

  // 构建连接
  const wsProvider = new WsProvider(args["port"]);
  const type_path = fs.readFileSync(args["type-file"]);
  const type_json = JSON.parse(type_path);
  // Create the API and wait until ready
  const api = await ApiPromise.create({
    provider: wsProvider,
    types: type_json,
  });

  const keyring = createTestKeyring();
  const adminPair = keyring.getPairs()[0];

  // 创建方法map
  var funcMap = {};
  funcMap["dbcPriceOcw"] = {};
  funcMap["dbcPriceOcw"]["addPriceUrl"] = api.tx.dbcPriceOcw.addPriceUrl;

  funcMap["committee"] = {};
  funcMap["committee"]["setStakedUsdPerOrder"] = api.tx.committee.setStakedUsdPerOrder;
  funcMap["committee"]["addCommittee"] = api.tx.committee.addCommittee;

  funcMap["leaseCommittee"] = {};
  funcMap["genericFunc"] = {};
  funcMap["genericFunc"]["setFixedTxFee"] = api.tx.genericFunc.setFixedTxFee;

  funcMap["onlineProfile"] = {};
  funcMap["onlineProfile"]["setGpuStake"] = api.tx.onlineProfile.setGpuStake;
  funcMap["onlineProfile"]["setRewardStartEra"] = api.tx.onlineProfile.setRewardStartEra;
  funcMap["onlineProfile"]["setPhaseNRewardPerEra"] = api.tx.onlineProfile.setPhaseNRewardPerEra;
  funcMap["onlineProfile"]["setStakeUsdLimit"] = api.tx.onlineProfile.setStakeUsdLimit;
  funcMap["onlineProfile"]["setStandardGpuPointPrice"] = api.tx.onlineProfile.setStandardGpuPointPrice;

  const callFunc = funcMap[args["module"]][args["func"]];
  await do_sign_tx(api, callFunc, adminPair, ...args._).catch((error) =>
    console.log(error.message)
  );
}

async function do_sign_tx(api, callFunc, adminPair, ...args) {
  const a = await api.tx.sudo
    .sudo(callFunc(...args))
    .signAndSend(adminPair, ({ events = [], status }) => {
      console.log(`{"Tx_status:":"${status.type}"}`);

      if (status.isInBlock) {
        console.log(`{"Tx_inBlock":"${status.asInBlock.toHex()}"}`);

        events.forEach(({ event: { data, method, section }, phase }) => {
          console.log(
            `{"Event":${phase.toString()},"func":"${section}.${method}","data":${data.toString()}}`
          );
        });
      } else if (status.isFinalized) {
        console.log(
          `{"Finalized_block_hash:":"${status.asFinalized.toHex()}"}`
        );
        process.exit(0);
      }
    });
}

main().catch((error) => console.log(error.message));
