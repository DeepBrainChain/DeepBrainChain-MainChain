// refer: https://polkadot.js.org/docs/substrate/extrinsics

// 使用： node sign_txs.js
// --port="wss://innertest.dbcwallet.io"
// --module=onlineProfile --func=bondMachine
// --key="sample split bamboo west visual approve brain fox arch impact relief smile"

// Import the API & Provider and some utility functions
import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import fs from "fs";
import minimist from "minimist";

// import the test keyring (already has dev keys for Alice, Bob, Charlie, Eve & Ferdie)
// kconst testKeyring = require('@polkadot/keyring/testing');

// Secret phrase `soldier until emotion future loop list crumble either voice select tattoo wife` is account:
//   Secret seed:       0xa8e5289227420a948ada4a550138326ab13c2aa879d468153b5a7edc98b35c11
//   SS58 Address:      5GbSsW5WQJWxvuyivg1pxzcCHDSZV8yrmgXSHtmMCdNyYmK7
// Secret phrase `bar vault anchor welcome unfold canyon calm pepper usage hint cube tissue` is account:
//   Secret seed:         0x9b3bf87d397b40c4daa2d381a801abb9c9b4e5f3dc6a4b3f529bdbd01f4670a5
//   SS58 Address:      5DMHnhL1cJ4y8jw7eEd6s38bF5CPA7cuELRPRgRZ5onhJGUN
// Secret phrase `deer hidden behave begin accuse barely mean radar river inflict razor belt` is account:
//   Secret seed:       0xe06b98f23f744049de53e9c5040622e9f5537606737dbfe5c9153b49931b4717
//   Public key (hex):  0x36ca7d9c55a6008eef1fd422ae890ee38e60017a4a2057d6b31726cf1d83ed55
//   SS58 Address:      5DJYbUba9ANpMBfzwnrG4jErxfg1gozhdfoLLsUbCBG7cV5o

async function main() {
  // 读取参数
  var args = minimist(process.argv.slice(2), {
    string: ["key1", "key2"],
  });

  // 构建连接
  const wsProvider = new WsProvider(args["port"]);
  // const rpc_path = fs.readFileSync(args["rpc-file"]);
  // const rpc_json = JSON.parse(rpc_path);

  // Create the API and wait until ready
  const api = await ApiPromise.create({
    provider: wsProvider,
    // types: type_json,
    // rpc: rpc_json,
  });

  // 读取密钥 type: sr25519, ssFormat: 42 (defaults)
  const keyring = new Keyring({ type: "sr25519" });
  // const accountFromKeyring = keyring.createFromUri(args["key"]); // 从助记词生成账户
  const accountFromKeyring1 = keyring.addFromUri(args["key1"]); // 从私钥生成账户对
  const accountFromKeyring2 = keyring.addFromUri(args["key2"]); // 从私钥生成账户对

  // 获取账户nonce
  const { nonce } = await api.query.system.account(accountFromKeyring1.address);
  const { nonce } = await api.query.system.account(accountFromKeyring2.address);

  var callFunc1 = api.tx.multisig.NewMultisig;
  var callFunc2 = api.tx.multisig.MultisigExecuted;

  // 第一次多签签名
  await do_sign_tx(callFunc1, accountFromKeyring1, nonce, ...args._).catch(
    (error) => console.log(error.message)
  );

  // 最后一次多签签名
  await do_sign_tx(callFunc2, accountFromKeyring2, nonce, ...args._).catch(
    (error) => console.log(error.message)
  );
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
