import { stringToU8a, u8aToHex, hexToU8a } from "@polkadot/util";
import minimist from "minimist";
import { naclSeal } from "@polkadot/util-crypto";

async function main() {
  // 读取参数
  var args = minimist(process.argv.slice(2), {
    string: ["sender_privkey", "receiver_box_pubkey", "msg"],
  });

  const nonce1 = new Uint8Array([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 11,
  ]);

  const { nonce, sealed } = naclSeal(
    stringToU8a(args["msg"]),
    hexToU8a(args["sender_privkey"]),
    hexToU8a(args["receiver_box_pubkey"]),
    nonce1
  );

  console.log(u8aToHex(sealed));
}

main().catch((error) => console.log(error.message));
