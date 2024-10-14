//! Environmental-aware externalities for EVM tracing in Wasm runtime. This enables
//! capturing the - potentially large - trace output data in the host and keep
//! a low memory footprint in `--execution=wasm`.
//!
//! - The original trace Runtime Api call is wrapped `using` environmental (thread local).
//! - Arguments are scale-encoded known types in the host.
//! - Host functions will decode the input and emit an event `with` environmental.

#![cfg_attr(not(feature = "std"), no_std)]
use sp_runtime_interface::runtime_interface;

use parity_scale_codec::Decode;
use sp_std::vec::Vec;

use dbc_primitives_rpc_evm_tracing_events::{
    Event, EvmEvent, GasometerEvent, RuntimeEvent, StepEventFilter,
};

#[runtime_interface]
pub trait TracingExt {
    fn raw_step(&mut self, _data: Vec<u8>) {}

    fn raw_gas(&mut self, _data: Vec<u8>) {}

    fn raw_return_value(&mut self, _data: Vec<u8>) {}

    fn call_list_entry(&mut self, _index: u32, _value: Vec<u8>) {}

    fn call_list_new(&mut self) {}

    // New design, proxy events.
    /// An `Evm` event proxied by the dbc runtime to this host function.
    /// evm -> dbc_runtime -> host.
    fn evm_event(&mut self, event: Vec<u8>) {
        if let Ok(event) = EvmEvent::decode(&mut &event[..]) {
            Event::Evm(event).emit();
        }
    }

    /// A `Gasometer` event proxied by the dbc runtime to this host function.
    /// evm_gasometer -> dbc_runtime -> host.
    fn gasometer_event(&mut self, event: Vec<u8>) {
        if let Ok(event) = GasometerEvent::decode(&mut &event[..]) {
            Event::Gasometer(event).emit();
        }
    }

    /// A `Runtime` event proxied by the dbc runtime to this host function.
    /// evm_runtime -> dbc_runtime -> host.
    fn runtime_event(&mut self, event: Vec<u8>) {
        if let Ok(event) = RuntimeEvent::decode(&mut &event[..]) {
            Event::Runtime(event).emit();
        }
    }

    /// Allow the tracing module in the runtime to know how to filter Step event
    /// content, as cloning the entire data is expensive and most of the time
    /// not necessary.
    fn step_event_filter(&self) -> StepEventFilter {
        dbc_primitives_rpc_evm_tracing_events::step_event_filter().unwrap_or_default()
    }

    /// An event to create a new CallList (currently a new transaction when tracing a block).
    #[version(2)]
    fn call_list_new(&mut self) {
        Event::CallListNew().emit();
    }
}
