import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import fs from "fs";
import minimist from "minimist";
import { createTestKeyring } from "@polkadot/keyring/testing";

async function main() {
  // 读取参数
  const args = minimist(process.argv.slice(2), { string: ["strinfo", "key"] });

  if (args.hasOwnProperty("strinfo")) {
    args._.push(args["strinfo"]);
  }

  // 构建连接
  const wsProvider = new WsProvider(args["port"]);
  const type_path = fs.readFileSync(args["type-file"]);
  const type_json = JSON.parse(type_path);
  const rpc_path = fs.readFileSync(args["rpc-file"]);
  const rpc_json = JSON.parse(rpc_path);

  // Create the API and wait until ready
  const api = await ApiPromise.create({
    provider: wsProvider,
    types: type_json,
    rpc: rpc_json,
  });

  // const keyring = createTestKeyring();
  // const adminPair = keyring.getPairs()[0];

  // 读取密钥 type: sr25519, ssFormat: 42 (defaults)
  const keyring = new Keyring({ type: "sr25519" });
  // const accountFromKeyring = keyring.createFromUri(args["key"]); // 从助记词生成账户
  const adminPair = keyring.addFromUri(args["key"]); // 从私钥生成账户对

  // 创建方法map
  var funcMap = {};
  funcMap["dbcPriceOcw"] = {};
  funcMap["dbcPriceOcw"]["addPriceUrl"] = api.tx.dbcPriceOcw.addPriceUrl;

  funcMap["committee"] = {};
  funcMap["committee"]["setStakedUsdPerOrder"] =
    api.tx.committee.setStakedUsdPerOrder;
  funcMap["committee"]["addCommittee"] = api.tx.committee.addCommittee;

  funcMap["onlineCommittee"] = {};
  funcMap["genericFunc"] = {};
  funcMap["genericFunc"]["setFixedTxFee"] = api.tx.genericFunc.setFixedTxFee;

  funcMap["onlineProfile"] = {};
  funcMap["onlineProfile"]["setGpuStake"] = api.tx.onlineProfile.setGpuStake;
  funcMap["onlineProfile"]["setRewardStartEra"] =
    api.tx.onlineProfile.setRewardStartEra;
  funcMap["onlineProfile"]["setPhaseNRewardPerEra"] =
    api.tx.onlineProfile.setPhaseNRewardPerEra;
  funcMap["onlineProfile"]["setStakeUsdLimit"] =
    api.tx.onlineProfile.setStakeUsdLimit;
  funcMap["onlineProfile"]["setStandardGpuPointPrice"] =
    api.tx.onlineProfile.setStandardGpuPointPrice;

  funcMap["onlineProfile"]["rootAddSysInfo"] =
    api.tx.onlineProfile.rootAddSysInfo;

  funcMap["rentMachine"] = {};
  funcMap["rentMachine"]["setRentPot"] = api.tx.rentMachine.setRentPot;

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
