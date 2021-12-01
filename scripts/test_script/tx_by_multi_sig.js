import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import fs from "fs";
import minimist from "minimist";
import { blake2AsHex } from "@polkadot/util-crypto";

// 不需要变
const typeFile = "../../dbc_types.json";
const callIndex = "0x0603";
const maxWeight = 194407000;

// 初始化一次即可
const websocket = "ws://127.0.0.1:9944";
const allAccount = [
  "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY",
  "5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty",
];
const threshold = 2;

// 每次转账之前设置
var destAccount = "5DAAnrj7VHTznn2AWBemMuyBwZWs6FNFjdyVXUeYum3PTXFy";
var transAmount = "10000000000000000"; // 10**15 * 10 = 10 DBC

// 第一次执行脚本之后设置
var firstCallHeight = 12;
var firstCallIndex = 1;
var isFirstCall = false;

var signerKey =
  "0xe5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a"; // 签名账户

// 最后一次执行时设置
var isFinalSign = false;

async function main() {
  // 读取参数
  var args = minimist(process.argv.slice(2), {
    string: ["key"],
  });

  // 构建连接
  const wsProvider = new WsProvider(websocket);
  const type_json = JSON.parse(fs.readFileSync(typeFile));

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

  var timepoint = null;
  if (!isFirstCall) {
    timepoint = { height: firstCallHeight, index: firstCallIndex };
  }

  // TODO: 当前改这个
  const otherAccount = ["5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty"];

  if (isFinalSign) {
    // 最后一次多签签名
    const callFunc = api.tx.multisig.asMulti;
    await do_sign_tx(
      callFunc,
      accountFromKeyring,
      nonce,
      threshold,
      otherAccount,
      { height: 32, index: 1 },
      encodedProposal2,
      false,
      maxWeight
    ).catch((error) => console.log(error.message));
  } else {
    // 第一次多签签名
    const callFunc = api.tx.multisig.approveAsMulti;
    await do_sign_tx(
      callFunc,
      accountFromKeyring,
      nonce,
      threshold,
      otherAccount,
      null,
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
