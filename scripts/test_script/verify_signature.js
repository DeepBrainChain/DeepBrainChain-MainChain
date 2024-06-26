// Example
//    node verify_signature.js \
//    --msg 143df6134aac849a446bc6a3460c2e06778161f3c0dc88cd299e358fd1e4232e5ELSwBWgRq5jN1j2YaP7qWRPTri6pKHpfpd7H4AePziXBabx \
//    --addr 5CXFDnKWPR17Mh3mWxoF25EPH2ehmy48r42jgAAfKCnaXo3A \
//    --sig 0xacd00b27caa33172f347fa2cf7b36dba5d34caee8900c0fe95a68a6ebaa5aa51300633318c00a0e873d0859966fbee8e54687bf2b5da8182e86dd9ef71dffe8e
// true

import { cryptoWaitReady, signatureVerify } from "@polkadot/util-crypto";
import minimist from "minimist";

async function main() {
  await cryptoWaitReady();

  const args = minimist(process.argv.slice(2), {
    string: ["msg", "sig", "addr"],
  });

  const { isValid } = signatureVerify(args["msg"], args["sig"], args["addr"]);
  console.log(isValid);
}

main().catch((error) => console.log(error.message));
