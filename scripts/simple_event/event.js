const DOT_DECIMAL_PLACES = 1000000000000;

// import {ApiPromise, WsProvider, Keyring} from '@polkadot/api';

const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');

(async () => {

    // const provider = new WsProvider('wss://kusama-rpc.polkadot.io/')
    const provider = new WsProvider('ws://127.0.0.1:9944/')
    const api = await ApiPromise.create({ provider })

    console.log(api.genesisHash.toHex());

    const lastHdr = await api.rpc.chain.getHeader();


    // const startHash = await api.rpc.chain.getBlockHash(666);
    // const endHash = await api.rpc.chain.getBlockHash(666);

    // const events = await api.query.system.events.range([startHash, endHash]);
    // events.forEach((event) => {
    //     console.log(`Event: ${JSON.stringify(event)}`);
    // });

    for (var i = 1; i < lastHdr.number.unwrap(); i++) {
        try {
            console.log("#####", i);
            const startHash = await api.rpc.chain.getBlockHash(i);
            // const endHash = await api.rpc.chain.getBlockHash(i + 5);

            const events = await api.query.system.events.range([startHash, startHash]);
            events.forEach((event) => {
                console.log(`Event: ${JSON.stringify(event)}`);
            });
        } catch (e) {
            console.log("#######", i, e);
        }
    }


    // const startHdr = await api.rpc.chain.getBlockHash(1);

    // // 查询最近 100 个块
    // const lastHdr = await api.rpc.chain.getHeader();
    // const startHdr = await api.rpc.chain.getBlockHash(lastHdr.number.unwrap().subn(1000));

    // console.log(startHdr);

    // const events = await api.query.system.events.range([startHdr]);

    // // // retrieve the range of events
    // // const events = await api.query.system.events.range([startHdr]);

    // events.forEach((event) => {
    //     console.log(`Event: ${JSON.stringify(event)}`);
    // });

    process.exit()
})()
