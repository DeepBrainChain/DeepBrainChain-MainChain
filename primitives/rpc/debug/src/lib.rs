#![cfg_attr(not(feature = "std"), no_std)]

use ethereum::{TransactionV0 as LegacyTransaction, TransactionV2 as Transaction};
use ethereum_types::H256;
use parity_scale_codec::{Decode, Encode};
use sp_std::vec::Vec;

sp_api::decl_runtime_apis! {
    // Api version is virtually 4.
    //
    // We realized that even using runtime overrides, using the ApiExt interface reads the api
    // versions from the state runtime, meaning we cannot just reset the versioning as we see fit.
    //
    // In order to be able to use ApiExt as part of the RPC handler logic we need to be always
    // above the version that exists on chain for this Api, even if this Api is only meant
    // to be used overridden.
    #[api_version(4)]
    pub trait DebugRuntimeApi {
        #[changed_in(4)]
        fn trace_transaction(
            extrinsics: Vec<Block::Extrinsic>,
            transaction: &LegacyTransaction,
        ) -> Result<(), sp_runtime::DispatchError>;

        fn trace_transaction(
            extrinsics: Vec<Block::Extrinsic>,
            transaction: &Transaction,
        ) -> Result<(), sp_runtime::DispatchError>;

        fn trace_block(
            extrinsics: Vec<Block::Extrinsic>,
            known_transactions: Vec<H256>,
        ) -> Result<(), sp_runtime::DispatchError>;
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, Encode, Decode)]
pub enum TracerInput {
    None,
    Blockscout,
    CallTracer,
}

/// DebugRuntimeApi V2 result. Trace response is stored in client and runtime api call response is
/// empty.
#[derive(Debug)]
pub enum Response {
    Single,
    Block,
}
