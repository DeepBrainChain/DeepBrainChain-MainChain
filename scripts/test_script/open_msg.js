import { u8aToHex, hexToU8a, u8aToString } from "@polkadot/util";
import minimist from "minimist";
import { naclOpen } from "@polkadot/util-crypto";

async function main() {
  // 读取参数
  var args = minimist(process.argv.slice(2), {
    string: ["sender_box_pubkey", "receiver_privkey", "sealed_msg"],
  });

  const nonce1 = new Uint8Array([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 11,
  ]);

  let opened = naclOpen(
    hexToU8a(args["sealed_msg"]),
    nonce1,
    hexToU8a(args["sender_box_pubkey"]),
    hexToU8a(args["receiver_privkey"])
  );

  console.log("Opened message is:", u8aToString(opened));
}

main().catch((error) => console.log(error.message));
