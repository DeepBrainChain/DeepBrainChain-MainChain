import { stringToU8a, u8aToHex } from "@polkadot/util";
import { cryptoWaitReady, signatureVerify } from "@polkadot/util-crypto";
import { Keyring } from "@polkadot/keyring";
import minimist from "minimist";

async function main() {
  await cryptoWaitReady();

  const args = minimist(process.argv.slice(2), { string: ["key"] });

  const keyring = new Keyring({ type: "sr25519" });
  const keyPair = keyring.addFromUri(args["key"]);

  // create the message, actual signature and verify
  const signature = keyPair.sign(stringToU8a(args["msg"]));

  console.log(`### Message: ${args["msg"]}`);
  console.log(`### Signer: ${u8aToHex(keyPair.publicKey).slice(2)}`);
  console.log(`### Signature: ${u8aToHex(signature)}`);

  // // verify the message using Alice's address
  // // const isValid = keyPair.verify(message, signature);
  // const { isValid } = signatureVerify(args["msg"], signature, keyPair.address);

  // // output the result
  // console.log(`${u8aToHex(signature)} is ${isValid ? "valid" : "invalid"}`);
}

main().catch(error => console.log(error.message));
