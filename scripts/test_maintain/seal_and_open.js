import { Keyring } from "@polkadot/keyring";
import minimist from "minimist";

// alice="5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY"
// alice_key="0xe5be9a5092b81bca64be81d212e7f2f9eba183bb7a90954f7b76361f6edb5c0a"

// bob="5FHneW46xGXgs5mUiveU4sbTyGBzmstUspZC92UhjJM694ty"
// bob_key="0x398f0c28f98885e046333d4a41c19cee4c37368a9832c6502f6cfd182e2aef89"

import {
  naclSeal,
  naclOpen,
  naclKeypairFromSeed,
  naclKeypairFromString,
  naclBoxKeypairFromSecret,
  deriveAddress,
} from "@polkadot/util-crypto";

async function main() {
  // 读取参数
  var args = minimist(process.argv.slice(2), {
    string: ["sender_key", "receiver_key", "msg"],
  });

  // naclSign
  const sender_hex_seed = Uint8Array.from(
    Buffer.from(args["sender_key"], "hex")
  );
  const sender = naclKeypairFromSeed(sender_hex_seed);
  const senderIdBoxKey = naclBoxKeypairFromSecret(sender.secretKey);

  const receiver_hex_seed = Uint8Array.from(
    Buffer.from(args["receiver_key"], "hex")
  );
  const receiver = naclKeypairFromSeed(receiver_hex_seed);
  const receiverIdBoxKey = naclBoxKeypairFromSecret(receiver.secretKey);

  // Sender encrypts message to send with the public key the receiver sent and send it to receiver
  // const message = new Uint8Array([1, 2, 3, 2, 1]);
  const message = string_to_u8a(args["msg"]);

  const nonce1 = new Uint8Array([
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 11,
  ]);

  const { nonce, sealed } = naclSeal(
    message,
    senderIdBoxKey.secretKey,
    receiverIdBoxKey.publicKey,
    nonce1
  );
  console.log(
    `Sender sends encrypted message to receiver ${sealed.toString()}, ${nonce.toString()}`
  );
  // Receiver opens encrypted message from the sender
  let opened = naclOpen(
    sealed,
    nonce,
    senderIdBoxKey.publicKey,
    receiverIdBoxKey.secretKey
  );
  console.log("Opened message is:", u8a_to_string(opened));
}

// https://github.com/michaelrhodes/u8a/blob/master/from-string.js
function string_to_u8a(str) {
  var l = str.length;
  var u8a = new Uint8Array(l);
  for (var i = 0; i < l; i++) {
    u8a[i] = str.charCodeAt(i);
  }
  return u8a;
}

function u8a_to_string(u8a) {
  var fromCharCode = String.fromCharCode;
  var chunk = 0x8000;
  var c = [];
  var l = u8a.length;
  for (var i = 0; i < l; i += chunk) {
    c.push(fromCharCode.apply(null, u8a.subarray(i, i + chunk)));
  }
  return c.join("");
}

main().catch((error) => console.log(error.message));
