import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
// import fs from "fs";
import minimist from "minimist";
import { blake2AsHex } from "@polkadot/util-crypto";

const websocket = "ws://127.0.0.1:9944";
const typeFile = "../../dbc_types.json";
const callIndex = "0x0603";
const maxWeight = 194407000;

const threshold = 2;
const allAccount = [];

var destAccount = "";
var transAmount = "";

var signerKey = ""; // 签名账户
var firstCallHeight = 12;
var firstCallIndex = 1;
var isFirstCall = fasle;
var isFinalSign = false;

async function main() {
  // 读取参数
  var args = minimist(process.argv.slice(2), {
    string: ["key"],
  });

  // 构建连接
  const wsProvider = new WsProvider(websocket);
  const type_json = JSON.parse(typeFile);

  // Create the API and wait until ready
  const api = await ApiPromise.create({
    provider: wsProvider,
    types: type_json,
  });

  // 从私钥生成账户对
  const keyring = new Keyring({ type: "sr25519" });
  const accountFromKeyring = keyring.addFromUri(signerKey);

  // 获取账户nonce
  const { nonce } = await api.query.system.account(accountFromKeyring.address);

  const callMethod = {
    callIndex,
    args: {
      dest: { id: destAccount },
      value: transAmount,
    },
  };
  const encodedProposal = api.createType("TransMethod", callMethod);
  const encodedProposal2 =
    "0x060300" + encodedProposal.toHex().toString().substring(6);
  const encodedHash = blake2AsHex(encodedProposal2);

  console.log("### encodedCall: ", encodedProposal2);
  console.log("### callHash: ", encodedHash);

  if (isFirstCall) {
    timepoint = null;
  } else {
    timepoint = { height: firstCallHeight, index: firstCallIndex };
  }

  // TODO:
  const otherAccount = allAccount;

  if (isFinalSign) {
    // 最后一次多签签名
    callFunc = api.tx.multisig.asMulti;
    await do_sign_tx(
      callFunc,
      accountFromKeyring2,
      nonce,
      threshold,
      [account1],
      { height: 32, index: 1 },
      encodedProposal2,
      false,
      maxWeight
    ).catch((error) => console.log(error.message));
  } else {
    // 第一次多签签名
    callFunc = api.tx.multisig.approveAsMulti;
    await do_sign_tx(
      callFunc1,
      accountFromKeyring1,
      nonce,
      threshold,
      [account2],
      timepoint,
      encodedHash,
      maxWeight
    ).catch((error) => console.log(error.message));
  }
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
          `{"#### Finalized_block_hash:":"${status.asFinalized.toHex()}"}`
        );
        process.exit(0);
      }
    }
  );
}

main().catch((error) => console.log(error.message));
