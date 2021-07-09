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
    rpc: rpc_json
  });

  // 当前：182696;
  // 发省时间：159898
  // 相差：22798

  const lastHdr = await api.rpc.chain.getHeader();
  const startHdr1 = await api.rpc.chain.getBlockHash(
    lastHdr.number.unwrap().subn(22805)
  );
  const startHdr2 = await api.rpc.chain.getBlockHash(
    lastHdr.number.unwrap().subn(22790)
  );

  console.log(startHdr1.toHuman(), startHdr2.toHuman());

  const momentPrev1 = await api.query.system.account.at(
    startHdr1,
    "5FC9eA9Bpk2a2qbLK9tkLwRA9frafgiHB9Jnn8cBweUyrWJu"
  );

  console.log(momentPrev1.toHuman());

  const momentPrev2 = await api.query.system.account.at(
    startHdr2,
    "5FC9eA9Bpk2a2qbLK9tkLwRA9frafgiHB9Jnn8cBweUyrWJu"
  );

  console.log(momentPrev2.toHuman());

  // const changes = await api.query.system.account.range([
  //   startHdr1,
  //   startHdr2
  // ])();

  // changes.forEach(([hash, value]) => {
  //   console.log(hash.toHex(), value.toHuman());
  // });
}

main().catch(error => console.log(error.message));
