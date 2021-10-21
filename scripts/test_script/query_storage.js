import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import fs from "fs";
import minimist from "minimist";

async function main() {
  // 读取参数
  const args = minimist(process.argv.slice(2));

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

  var funcMap = {};
  funcMap["staking"] = {};
  funcMap["staking"]["ledger"] = api.query.staking.ledger;

  funcMap["balances"] = {};
  funcMap["balances"]["locks"] = api.query.balances.locks;

  funcMap["system"] = {};
  funcMap["system"]["account"] = api.query.system.account;

  funcMap["dbcPriceOcw"] = {};
  funcMap["dbcPriceOcw"]["avgPrice"] = api.query.dbcPriceOcw.avgPrice;
  funcMap["dbcPriceOcw"]["priceURL"] = api.query.dbcPriceOcw.priceURL;

  funcMap["committee"] = {};
  funcMap["committee"]["committeeStakeDBCPerOrder"] =
    api.query.committee.committeeStakeDBCPerOrder;

  funcMap["onlineCommittee"] = {};
  funcMap["onlineCommittee"]["committeeMachine"] =
    api.query.onlineCommittee.committeeMachine;

  funcMap["genericFunc"] = {};
  funcMap["genericFunc"]["fixedTxFee"] = api.query.genericFunc.fixedTxFee;

  funcMap["onlineProfile"] = {};
  funcMap["onlineProfile"]["stakePerGPU"] = api.query.onlineProfile.stakePerGPU;
  funcMap["onlineProfile"]["stashMachines"] =
    api.query.onlineProfile.stashMachines;
  funcMap["onlineProfile"]["sysInfo"] = api.query.onlineProfile.sysInfo;
  funcMap["onlineProfile"]["machinesInfo"] =
    api.query.onlineProfile.machinesInfo;
  funcMap["onlineProfile"]["erasMachinePoints"] =
    api.query.onlineProfile.erasMachinePoints;
  funcMap["onlineProfile"]["erasStashPoints"] =
    api.query.onlineProfile.erasStashPoints;

  funcMap["rentMachine"] = {};
  funcMap["rentMachine"]["userTotalStake"] =
    api.query.rentMachine.userTotalStake;
  funcMap["rentMachine"]["userRented"] = api.query.rentMachine.userRented;
  funcMap["rentMachine"]["rentOrder"] = api.query.rentMachine.rentOrder;

  funcMap["maintainCommittee"] = {};
  funcMap["maintainCommittee"]["committeeOps"] =
    api.query.maintainCommittee.committeeOps;

  let heightHash = await api.rpc.chain.getBlockHash(args["at-height"]);

  var callFunc = funcMap[args["module"]][args["func"]].at;
  await do_query(callFunc, heightHash, ...args._).catch((error) =>
    console.log(error.message)
  );
}

async function do_query(callFunc, heightHash, ...args) {
  const a = await callFunc(heightHash, ...args);
  console.log(a.toString());
  // console.log(`${a.machine_info_detail.staker_customize_info}`);
  process.exit(0);
}

main().catch((error) => console.log(error.message));
