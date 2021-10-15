#![cfg_attr(not(feature = "std"), no_std)]

use codec::EncodeLike;
use frame_support::{
    pallet_prelude::*,
    traits::{Currency, ReservableCurrency},
};
use frame_system::pallet_prelude::*;
use online_profile::StashMachine;
use online_profile_machine::OPRPCQuery;
pub use pallet::*;
use pallet_identity::Data;
use sp_std::vec::Vec;

pub mod rpc_types;
pub use rpc_types::*;

type BalanceOf<T> = <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + pallet_identity::Config {
        type Currency: ReservableCurrency<Self::AccountId>;
        type OPRpcQuery: OPRPCQuery<AccountId = Self::AccountId, StashMachine = StashMachine<BalanceOf<Self>>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::call]
    impl<T: Config> Pallet<T> {}
}

impl<T: Config> Module<T> {
    pub fn get_staker_identity(account: impl EncodeLike<T::AccountId>) -> Vec<u8> {
        let account_info = <pallet_identity::Module<T>>::identity(account);
        if let None = account_info {
            return Vec::new()
        }
        let account_info = account_info.unwrap();

        match account_info.info.display {
            Data::Raw(out) => return out,
            _ => return Vec::new(),
        }
    }

    // 返回total_page
    pub fn get_staker_list_info(cur_page: u64, per_page: u64) -> Vec<StakerListInfo<BalanceOf<T>, T::AccountId>> {
        let all_stash = T::OPRpcQuery::get_all_stash();
        let mut stash_list_info = Vec::new();

        if all_stash.len() == 0 {
            return stash_list_info
        }

        let cur_page = cur_page as usize;
        let per_page = per_page as usize;
        let page_start = cur_page * per_page;
        let mut page_end = page_start + per_page;

        if page_start >= all_stash.len() {
            return stash_list_info
        }

        if page_end >= all_stash.len() {
            page_end = all_stash.len();
        }

        if page_start > page_end {
            page_end = page_start;
        }

        for (index, a_stash) in all_stash.into_iter().enumerate() {
            let staker_info = T::OPRpcQuery::get_stash_machine(a_stash.clone());
            let identity = Self::get_staker_identity(a_stash.clone());

            stash_list_info.push(StakerListInfo {
                index: index as u64 + 1,
                staker_name: identity,
                staker_account: a_stash.clone(),
                calc_points: staker_info.total_calc_points,
                total_gpu_num: staker_info.total_gpu_num,
                total_rented_gpu: staker_info.total_rented_gpu,
                total_rent_fee: staker_info.total_rent_fee,
                total_burn_fee: staker_info.total_burn_fee,
                total_reward: staker_info.total_earned_reward,
                total_released_reward: staker_info.total_claimed_reward + staker_info.can_claim_reward,
            })
        }

        stash_list_info.sort_by(|a, b| b.calc_points.cmp(&a.calc_points));

        let item_len = stash_list_info.len();
        for index in 0..item_len {
            stash_list_info[index].index = index as u64;
        }

        stash_list_info[page_start..page_end].to_vec()
    }
}
