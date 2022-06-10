// refer: https://polkadot.js.org/docs/substrate/extrinsics

// Usage： node sign_tx_by_user.js
// --port="wss://info.dbcwallet.io"
// --module=onlineProfile --func=bondMachine
// --key="sample split bamboo west visual approve brain fox arch impact relief smile"

// Import the API & Provider and some utility functions
import { ApiPromise, WsProvider } from "@polkadot/api";
import { Keyring } from "@polkadot/keyring";
import fs from "fs";
import minimist from "minimist";

// import the test keyring (already has dev keys for Alice, Bob, Charlie, Eve & Ferdie)
// kconst testKeyring = require('@polkadot/keyring/testing');

async function main() {
  // Read params from command line
  var args = minimist(process.argv.slice(2), {
    string: ["key", "sig", "hash"],
  });

  if (args.hasOwnProperty("sig")) {
    args._.push(args["sig"]);
  }

  if (args.hasOwnProperty("hash")) {
    args._.push(args["hash"]);
  }

  // Create connection
  const wsProvider = new WsProvider(args["port"]);
  const type_path = fs.readFileSync(args["type-file"]);
  const type_json = JSON.parse(type_path);
  const rpc_path = fs.readFileSync(args["rpc-file"]);
  const rpc_json = JSON.parse(rpc_path);

  // Create the API and wait until ready
  const api = await ApiPromise.create({
    provider: wsProvider,
    types: type_json,
    rpc: rpc_json,
  });

  // Read account key: type: sr25519, ssFormat: 42 (defaults)
  const keyring = new Keyring({ type: "sr25519" });
  // const accountFromKeyring = keyring.createFromUri(args["key"]); // Generate Account from Mnemonic Phrase
  const accountFromKeyring = keyring.addFromUri(args["key"]); // Generate account pair from private key

  // Get account nonce
  const { nonce } = await api.query.system.account(accountFromKeyring.address);

  // Create function you want to call
  var funcMap = {};
  funcMap["onlineProfile"] = {};
  funcMap["onlineProfile"]["setController"] =
    api.tx.onlineProfile.setController;
  funcMap["onlineProfile"]["machineSetStash"] =
    api.tx.onlineProfile.machineSetStash;
  funcMap["onlineProfile"]["claimRewards"] = api.tx.onlineProfile.claimRewards;

  funcMap["onlineProfile"]["bondMachine"] = api.tx.onlineProfile.bondMachine;

  funcMap["committee"] = {};
  funcMap["committee"]["committeeSetBoxPubkey"] =
    api.tx.committee.committeeSetBoxPubkey;

  funcMap["onlineCommittee"] = {};
  funcMap["onlineCommittee"]["submitConfirmHash"] =
    api.tx.onlineCommittee.submitConfirmHash;
  funcMap["onlineCommittee"]["submitConfirmRaw"] =
    api.tx.onlineCommittee.submitConfirmRaw;

  funcMap["rentMachine"] = {};
  funcMap["rentMachine"]["rentMachine"] = api.tx.rentMachine.rentMachine;
  funcMap["rentMachine"]["confirmRent"] = api.tx.rentMachine.confirmRent;
  funcMap["rentMachine"]["reletMachine"] = api.tx.rentMachine.reletMachine;

  funcMap["balances"] = {};
  funcMap["balances"]["transfer"] = api.tx.balances.transfer;

  var callFunc = funcMap[args["module"]][args["func"]];
  await do_sign_tx(callFunc, accountFromKeyring, nonce, ...args._).catch(
    (error) => console.log(error.message)
  );
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
          `{"Finalized_block_hash:":"${status.asFinalized.toHex()}"}`
        );
        process.exit(0);
      }
    }
  );
}

main().catch((error) => console.log(error.message));
