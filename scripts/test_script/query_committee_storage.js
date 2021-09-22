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
  funcMap["onlineCommittee"] = {};
  funcMap["onlineCommittee"]["committeeMachine"] =
    api.query.onlineCommittee.committeeMachine;
  funcMap["onlineCommittee"]["machineCommittee"] =
    api.query.onlineCommittee.machineCommittee;

  var callFunc = funcMap[args["module"]][args["func"]];
  await do_query(callFunc, ...args._).catch((error) =>
    console.log(error.message)
  );
}

async function do_query(callFunc, ...args) {
  const a = await callFunc(...args);
  console.log(a.toJSON());

  process.exit(0);
}

main().catch((error) => console.log(error.message));
