# machine status changed events

* MachineExit(MachineId) - triggered when a machine exits the system.
* MachineOfflineToMutHardware(MachineId, BalanceOf<T>, BalanceOf<T>) - triggered when a machine transitions from online to offline to mutual hardware.
* ControllerReportOffline(MachineId), - triggered when a machine is reported to offline by a controller.
* ControllerReportOnline(MachineId), - triggered when a machine is reported to online by a controller.
* BondMachine(T::AccountId, MachineId, BalanceOf<T>), - triggered when a new machine is bonded to an account.
* ConfirmRent(RentOrderId, T::AccountId, MachineId, u32, T::BlockNumber, BalanceOf<T>) - triggered when a rent order is confirmed.


# you can subscribe to these events using the following js code:

```javascript
// Import the API
const { ApiPromise, WsProvider } = require('@polkadot/api');

async function main () {
    const wsProvider = new WsProvider('ws://127.0.0.1:8000');

    const api = await ApiPromise.create({provider:wsProvider});

    // Subscribe to system events via storage
    api.query.system.events((events) => {
        console.log(`\nReceived ${events.length} events:`);
    
        // Loop through the Vec<EventRecord>
        events.forEach((record) => {
            console.log(`new event received`);

            // Extract the event
            const {event} = record;
            // Show what we are busy with
            console.log(`\t${event.section}:${event.method}`);
            if (event.section === 'rentMachine' && event.method === 'ConfirmRent') {
                console.log(`\tfind Rent confirmed for machine`);

                // Extract the event types
                const types = event.typeDef;
                
                // Loop through each of the parameters, displaying the type and data
                event.data.forEach((data, index) => {
                console.log(` ${types[index].type}: ${data.toString()}`);
                });
            }
        });
    });
}

```
useful link: https://polkadot.js.org/docs/api/examples/promise/system-events
