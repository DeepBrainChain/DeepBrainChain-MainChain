const DOT_DECIMAL_PLACES = 1000000000000;

const { ApiPromise, WsProvider, Keyring } = require('@polkadot/api');

(async () => {
    const provider = new WsProvider('ws://127.0.0.1:9944/')
    const api = await ApiPromise.create({ provider })

    console.log(api.genesisHash.toHex());

    const lastHdr = await api.rpc.chain.getHeader();

    for (var i = 1; i < lastHdr.number.unwrap(); i++) {
        try {
            const startHash = await api.rpc.chain.getBlockHash(i);
            // const lastHdr = await api.rpc.chain.getHeader();
            // const endHash = await api.rpc.chain.getBlockHash(i + 5);

            const events = await api.query.system.events.range([startHash, startHash]);
            events.forEach((event) => {
                console.log(`Event: ${JSON.stringify(event)}`);
            });
        } catch (e) {
            console.log(`Event: ${i}, ${e}`);
        }
    }

    process.exit(0)
})()