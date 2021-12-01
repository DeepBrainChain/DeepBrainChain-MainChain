// refer: https://polkadot.js.org/docs/substrate/extrinsics

// 使用： node sign_txs.js
// --port="wss://innertest.dbcwallet.io"
// --module=onlineProfile --func=bondMachine
// --key="sample split bamboo west visual approve brain fox arch impact relief smile"

// Import the API & Provider and some utility functions
import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
// import { useApi, useCall } from "@polkadot/react-hooks";
import fs from "fs";
import minimist from "minimist";
import { blake2AsHex } from "@polkadot/util-crypto";

async function main() {
  // 读取参数
  var args = minimist(process.argv.slice(2), {
    string: ["key1", "key2"],
  });

  // 构建连接
  const wsProvider = new WsProvider(args["port"]);
  const type_path = fs.readFileSync(args["type-file"]);
  const type_json = JSON.parse(type_path);

  // Create the API and wait until ready
  const api = await ApiPromise.create({
    provider: wsProvider,
    types: type_json,
  });

  const account1 = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY";
  const key1 =
    "0xe5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a";
  const account2 = "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty";
  const key2 =
    "0x398f0c28f98885e046333d4a41c19cee4c37368a9832c6502f6cfd182e2aef89";

  // 读取密钥 type: sr25519, ssFormat: 42 (defaults)
  const keyring1 = new Keyring({ type: "sr25519" });
  // const accountFromKeyring = keyring.createFromUri(args["key"]); // 从助记词生成账户
  const accountFromKeyring1 = keyring1.addFromUri(key1); // 从私钥生成账户对

  const keyring2 = new Keyring({ type: "sr25519" });
  const accountFromKeyring2 = keyring2.addFromUri(key2); // 从私钥生成账户对

  // 获取账户nonce
  const { nonce1 } = await api.query.system.account(
    accountFromKeyring1.address
  );
  const { nonce2 } = await api.query.system.account(
    accountFromKeyring2.address
  );

  // TODO: Change here
  const threshold = 2;
  const maxWeight = 194407000;
  // TOOD: Change here
  const method = {
    callIndex: "0x0603",
    args: {
      dest: { id: "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY" },
      value: "0x0000000000000000002386f26fc10000",
    },
  };
  const encodedProposal = api.createType("TransMethod", method);

  console.log("########## should be right: ", encodedProposal.toHex());

  const encodedProposal2 =
    "0x060300" + encodedProposal.toHex().toString().substring(6);
  console.log("### encodedProposal...", encodedProposal2);
  const encodedHash = blake2AsHex(encodedProposal2);
  console.log("hash: ", encodedHash);

  const method2 = {
    callIndex: "0x0603",
    callModule: "balance",
    callName: "transferKeepAlive",
    params: {
      dest: { id: "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY" },
      value: "0x0000000000000000002386f26fc10000",
    },
  };

  // const encodeCall2 =
  //   "0x060300" + api.createType("TransMethod2", method2).toHex().toString();

  const encodeCall2 = api.createType("TransMethod2", method2).toHex();
  // .toString();

  console.log("###########", encodeCall2);

  var callFunc1 = api.tx.multisig.approveAsMulti;
  var callFunc2 = api.tx.multisig.asMulti;

  // 第一次多签签名
  await do_sign_tx(
    callFunc1,
    accountFromKeyring1,
    nonce1,
    threshold,
    [account2],
    null,
    encodedHash,
    maxWeight
  ).catch((error) => console.log(error.message));

  // // 最后一次多签签名
  // await do_sign_tx(
  //   callFunc2,
  //   accountFromKeyring2,
  //   nonce2,
  //   threshold,
  //   [account1],
  //   { height: 32, index: 1 },
  //   encodedProposal2,
  //   false,
  //   maxWeight
  // ).catch((error) => console.log(error.message));
}

async function do_sign_tx(callFunc, accountFromKeyring, nonce, ...args) {
  const a = await callFunc(...args).signAndSend(
    accountFromKeyring,
    { nonce },
    ({ events = [], status }) => {
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
    }
  );
}

main().catch((error) => console.log(error.message));
