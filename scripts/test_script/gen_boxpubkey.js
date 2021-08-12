import { hexToU8a, u8aToHex } from "@polkadot/util";
import { cryptoWaitReady } from "@polkadot/util-crypto";
import minimist from "minimist";

import {
  naclKeypairFromSeed,
  naclBoxKeypairFromSecret,
} from "@polkadot/util-crypto";

async function main() {
  await cryptoWaitReady();

  const args = minimist(process.argv.slice(2), { string: ["key"] });

  const keyPair = naclKeypairFromSeed(hexToU8a(args["key"]));
  const boxKeyPair = naclBoxKeypairFromSecret(keyPair.secretKey);
  console.log(u8aToHex(boxKeyPair.publicKey));
}

main().catch((error) => console.log(error.message));
