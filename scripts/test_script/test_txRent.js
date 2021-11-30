import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import { cryptoWaitReady } from "@polkadot/util-crypto";
import minimist from "minimist";
const node = {
  dbc: "wss://info.dbcwallet.io",
};

let api = null;
const keyring = new Keyring({ type: "sr25519" });
const args = minimist(process.argv.slice(2), { string: ["key", "day"] });
// 链上交互
export const GetApi = async () => {
  if (!api) {
    const provider = new WsProvider(node.dbc);
    api = await ApiPromise.create({ provider });
  }
  return { api };
};

let machineList = [
  "285ea26fa50016eb9f0fbe854ddfd34ba7966b7f91d96249f1948fafa718aa6a",
  "d6a8a8d90451e6ded3719e02fe9d1e3ffd92d1bc52a470076a0f8f3aa264fc62",
  "fcfd705589318e5554abc0b7318626720c7a319459a238d86dd37cf94995963b",
  "029cac37fb61744a0ace988ba37c4094f0803f54d0ef125866a8926932ce9e64",
  "249c3438aeb1311178dced2c2c26c1b29269dd209fbf0f385576985b5c769569",
  "3e4831b6e98738a4cf11e4797f940760bbf86df56b7535963aebb4c13a966104",
  "16bd6ff46e0f6d3d9edcd0654e0cdbe323344663f4a47c206c0f4dfdac015711",
  "3a557998c51fd4d8a33f2eb09a06096a8ea65bc32297fa6983e31c463058d25b",
  "d0b5a7ce51aab80b01aa81f2faac8a5a59ad53515f764a29f213df160a8e147b",
  "38f4a824e0dc1fc5a9a7dccff53417b300fc0edad208176d8770597d98f6eb5c",
];

export const utility = async (value) => {
  await GetApi();
  let newArray = machineList.map((res) => {
    return api.tx.rentMachine.rentMachine(res, value);
  });
  let accountFromKeyring = await keyring.addFromUri(args["key"]);
  await cryptoWaitReady();
  await api.tx.utility
    .batch(newArray)
    .signAndSend(
      accountFromKeyring,
      async ({ events = [], status, dispatchError }) => {
        console.log(`{"Tx_status:":"${status.type}"}`);
        if (status.isInBlock) {
          events.forEach(
            async ({
              event: {
                method,
                data: [error],
              },
            }) => {
              // console.log(method, error, error.words, 'method');
              if (method == "BatchInterrupted") {
                // const decoded = api?.registry.findMetaError(error.asModule);
                console.log("ExtrinsicFiles--->" + "成功执行：" + error.words);
              } else if (method == "BatchCompleted") {
                console.log("ExtrinsicSuccess: 全部执行");
              }
            }
          );
          if (status.isInBlock) {
            console.log(`included in ${status.asInBlock}`);
          }
        }
      }
    );
};
utility(args["day"]).catch((error) => console.log(error.message));
