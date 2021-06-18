import { hexToU8a, stringToU8a, u8aToHex } from "@polkadot/util";
import { cryptoWaitReady, signatureVerify } from "@polkadot/util-crypto";
import { Keyring } from "@polkadot/keyring";

import { ApiPromise, WsProvider } from "@polkadot/api";
import fs from "fs";
import minimist from "minimist";
import { createTestKeyring } from "@polkadot/keyring/testing";

import {
  naclSeal,
  naclOpen,
  naclKeypairFromSeed,
  naclKeypairFromString,
  naclBoxKeypairFromSecret,
  deriveAddress,
} from "@polkadot/util-crypto";


async function main() {
  await cryptoWaitReady();

  // 读取参数
  const args = minimist(process.argv.slice(2), { string: ["key"] });

  // naclSign
  const keyPair = naclKeypairFromSeed(hexToU8a(args["key"])); // 出错
  const boxKeyPair = naclBoxKeypairFromSecret(keyPair.secretKey);
  console.log(u8aToHex(boxKeyPair.publicKey));
}

main().catch((error) => console.log(error.message));
