#![cfg_attr(not(feature = "std"), no_std)]
// These clippy lints are disabled because the macro-generated code triggers them.
#![allow(clippy::unnecessary_mut_passed)]
#![allow(clippy::too_many_arguments)]

pub use ethereum::{TransactionV0 as LegacyTransaction, TransactionV2 as Transaction};
use parity_scale_codec::{Decode, Encode};
use sp_runtime::{scale_info::TypeInfo, traits::Block as BlockT, RuntimeDebug};
use sp_std::vec::Vec;

#[derive(Eq, PartialEq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct TxPoolResponseLegacy {
    pub ready: Vec<LegacyTransaction>,
    pub future: Vec<LegacyTransaction>,
}

#[derive(Eq, PartialEq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct TxPoolResponse {
    pub ready: Vec<Transaction>,
    pub future: Vec<Transaction>,
}

sp_api::decl_runtime_apis! {
    #[api_version(2)]
    pub trait TxPoolRuntimeApi {
        #[changed_in(2)]
        fn extrinsic_filter(
            xt_ready: Vec<<Block as BlockT>::Extrinsic>,
            xt_future: Vec<<Block as BlockT>::Extrinsic>,
        ) -> TxPoolResponseLegacy;
        fn extrinsic_filter(
            xt_ready: Vec<<Block as BlockT>::Extrinsic>,
            xt_future: Vec<<Block as BlockT>::Extrinsic>,
        ) -> TxPoolResponse;
    }
}
