#![cfg_attr(not(feature = "std"), no_std)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::unnecessary_mut_passed)]
#![warn(unused_crate_dependencies)]

use codec::Codec;
use simple_rpc::StakerListInfo;
use sp_std::prelude::Vec;

sp_api::decl_runtime_apis! {
    pub trait SimpleRpcApi<AccountId, Balance> where
        AccountId: Codec,
        Balance: Codec
    {
        fn get_staker_identity(account: AccountId) -> Vec<u8>;
        fn get_staker_list_info(cur_page: u64, per_page: u64) -> Vec<StakerListInfo<Balance, AccountId>>;
    }
}
