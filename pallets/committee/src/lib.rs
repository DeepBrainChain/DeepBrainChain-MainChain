#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

// pub mod migrations;
mod rpc;
mod traits;
mod types;
// #[allow(clippy::all)]
// pub mod weights;

#[cfg(any(feature = "runtime-benchmarks", test))]
mod benchmarking;
#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use dbc_support::ItemList;
use frame_support::{
    ensure,
    pallet_prelude::*,
    traits::{Currency, ReservableCurrency},
};
use frame_system::pallet_prelude::*;
use sp_runtime::traits::{CheckedAdd, CheckedSub, Saturating, Zero};
use sp_std::{prelude::*, str};

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub use pallet::*;
pub use traits::*;
pub use types::*;
// pub use weights::WeightInfo;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: ReservableCurrency<Self::AccountId>;
        // type WeightInfo: WeightInfo;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::storage]
    #[pallet::getter(fn committee)]
    pub(super) type Committee<T: Config> = StorageValue<_, CommitteeList<T::AccountId>, ValueQuery>;

    /// 委员会质押模块基本参数
    #[pallet::storage]
    #[pallet::getter(fn committee_stake_params)]
    pub(super) type CommitteeStakeParams<T: Config> =
        StorageValue<_, CommitteeStakeParamsInfo<BalanceOf<T>>>;

    /// 委员会质押与收益情况
    #[pallet::storage]
    #[pallet::getter(fn committee_stake)]
    pub(super) type CommitteeStake<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, CommitteeStakeInfo<BalanceOf<T>>, ValueQuery>;

    // The current storage version.
    #[pallet::storage]
    #[pallet::getter(fn storage_version)]
    pub(super) type StorageVersion<T: Config> = StorageValue<_, u16, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置committee每次操作需要质押数量
        #[pallet::call_index(0)]
        #[pallet::weight(0)]
        pub fn set_committee_stake_params(
            origin: OriginFor<T>,
            stake_params: CommitteeStakeParamsInfo<BalanceOf<T>>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            CommitteeStakeParams::<T>::put(stake_params);
            Ok(().into())
        }

        // 该操作由社区决定
        // 添加到委员会，直接添加到fulfill列表中。每次finalize将会读取委员会币数量，
        // 币足则放到committee中 TODO: add max_committee config for better weight
        #[pallet::call_index(1)]
        #[pallet::weight(10000)]
        // #[pallet::weight(<T as Config>::WeightInfo::add_committee(100u32))]
        pub fn add_committee(
            origin: OriginFor<T>,
            member: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let mut committee = Self::committee();
            // 确保用户还未加入到本模块
            ensure!(!committee.is_committee(&member), Error::<T>::AccountAlreadyExist);
            // 将用户添加到waiting_box_pubkey列表中

            ItemList::add_item(&mut committee.waiting_box_pubkey, member.clone());
            Committee::<T>::put(committee);

            Self::deposit_event(Event::CommitteeAdded(member));
            Ok(().into())
        }

        /// 委员会添用于非对称加密的公钥信息，并绑定质押
        #[pallet::call_index(2)]
        #[pallet::weight(10000)]
        pub fn committee_set_box_pubkey(
            origin: OriginFor<T>,
            box_pubkey: [u8; 32],
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let committee_stake_params =
                Self::committee_stake_params().ok_or(Error::<T>::GetStakeParamsFailed)?;

            let mut committee_list = Self::committee();

            // 只允许waiting_puk, normal 执行
            if committee_list.is_waiting_puk(&committee) {
                Self::check_stake_health(&committee, committee_stake_params.stake_baseline, true)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;
                Self::do_change_reserved(
                    committee.clone(),
                    committee_stake_params.stake_baseline,
                    true,
                    true,
                )
                .map_err(|_| Error::<T>::BalanceNotEnough)?;

                ItemList::rm_item(&mut committee_list.waiting_box_pubkey, &committee);
                ItemList::add_item(&mut committee_list.normal, committee.clone());
                Committee::<T>::put(committee_list);
            } else if !committee_list.is_normal(&committee) {
                return Err(Error::<T>::StatusNotAllowed.into())
            }

            CommitteeStake::<T>::mutate(&committee, |committee_stake| {
                committee_stake.box_pubkey = box_pubkey;
            });

            Self::deposit_event(Event::StakeAdded(
                committee.clone(),
                committee_stake_params.stake_baseline,
            ));
            Self::deposit_event(Event::CommitteeSetBoxPubkey(committee, box_pubkey));
            Ok(().into())
        }

        /// 委员会增加质押
        #[pallet::call_index(3)]
        #[pallet::weight(10000)]
        pub fn committee_add_stake(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let mut committee_list = Self::committee();
            ensure!(committee_list.is_committee(&committee), Error::<T>::NotCommittee);

            // 增加委员会质押
            Self::check_stake_health(&committee, amount, true)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;
            Self::do_change_reserved(committee.clone(), amount, true, true)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;

            if committee_list.is_fulfilling(&committee) {
                ItemList::rm_item(&mut committee_list.fulfilling_list, &committee);
                ItemList::add_item(&mut committee_list.normal, committee.clone());
                Committee::<T>::put(committee_list);
            }

            Self::deposit_event(Event::StakeAdded(committee, amount));
            Ok(().into())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(10000)]
        pub fn committee_reduce_stake(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let committee_list = Self::committee();
            ensure!(committee_list.is_normal(&committee), Error::<T>::NotInNormalList);

            // 减少委员会质押
            Self::check_stake_health(&committee, amount, false)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;
            Self::do_change_reserved(committee.clone(), amount, false, true)
                .map_err(|_| Error::<T>::ChangeReservedFailed)?;

            Self::deposit_event(Event::StakeReduced(committee, amount));
            Ok(().into())
        }

        #[pallet::call_index(5)]
        #[pallet::weight(10000)]
        pub fn claim_reward(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;

            let mut committee_stake = Self::committee_stake(&committee);
            ensure!(committee_stake.can_claim_reward != Zero::zero(), Error::<T>::NothingToClaim);

            let can_claim_reward = committee_stake.can_claim_reward;
            committee_stake.claimed_reward += can_claim_reward;
            committee_stake.can_claim_reward = Zero::zero();

            <T as Config>::Currency::deposit_into_existing(&committee, can_claim_reward)
                .map_err(|_| Error::<T>::ClaimRewardFailed)?;

            CommitteeStake::<T>::insert(&committee, committee_stake);
            Self::deposit_event(Event::ClaimReward(committee, can_claim_reward));
            Ok(().into())
        }

        // 委员会停止接单
        #[pallet::call_index(6)]
        #[pallet::weight(10000)]
        pub fn chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let mut committee_list = Self::committee();

            ensure!(committee_list.is_committee(&committee), Error::<T>::NotCommittee);
            if committee_list.is_chill(&committee) {
                return Ok(().into())
            }
            // waiting_box_pubkey不能执行该操作
            ensure!(!committee_list.is_waiting_puk(&committee), Error::<T>::PubkeyNotSet);

            // Allow normal & fulfilling committee to chill
            ItemList::rm_item(&mut committee_list.normal, &committee);
            ItemList::rm_item(&mut committee_list.fulfilling_list, &committee);
            ItemList::add_item(&mut committee_list.chill_list, committee.clone());

            Committee::<T>::put(committee_list);
            Self::deposit_event(Event::Chill(committee));
            Ok(().into())
        }

        // 委员会可以接单
        #[pallet::call_index(7)]
        #[pallet::weight(10000)]
        pub fn undo_chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let mut committee_list = Self::committee();

            ensure!(committee_list.is_chill(&committee), Error::<T>::NotInChillList);

            ItemList::rm_item(&mut committee_list.chill_list, &committee);
            ItemList::add_item(&mut committee_list.normal, committee.clone());

            let _ = Self::do_change_status_when_stake_changed(
                committee.clone(),
                &mut committee_list,
                &Self::committee_stake(&committee),
            );

            Committee::<T>::put(committee_list);
            Self::deposit_event(Event::UndoChill(committee));
            Ok(().into())
        }

        /// Only In Chill list & used_stake == 0 can exit.
        #[pallet::call_index(8)]
        #[pallet::weight(10000)]
        pub fn exit_committee(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let mut committee_stake = Self::committee_stake(&committee);
            let mut committee_list = Self::committee();

            ensure!(committee_stake.used_stake == Zero::zero(), Error::<T>::JobNotDone);
            ensure!(committee_list.is_chill(&committee), Error::<T>::StatusNotFeat);

            ItemList::rm_item(&mut committee_list.chill_list, &committee);
            let _ = <T as Config>::Currency::unreserve(&committee, committee_stake.staked_amount);

            committee_stake.staked_amount = Zero::zero();

            CommitteeStake::<T>::insert(&committee, committee_stake);
            Committee::<T>::put(committee_list);
            Self::deposit_event(Event::ExitFromCandidacy(committee));
            Ok(().into())
        }
    }

    #[pallet::event]
    // #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        PayTxFee(T::AccountId, BalanceOf<T>),
        CommitteeAdded(T::AccountId),
        CommitteeFulfill(BalanceOf<T>),
        Chill(T::AccountId),
        CommitteeExit(T::AccountId),
        UndoChill(T::AccountId),
        ExitFromCandidacy(T::AccountId),
        CommitteeSetBoxPubkey(T::AccountId, [u8; 32]),
        StakeAdded(T::AccountId, BalanceOf<T>),
        StakeReduced(T::AccountId, BalanceOf<T>),
        ClaimReward(T::AccountId, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        PubkeyNotSet,
        NotCommittee,
        AccountAlreadyExist,
        NotInChillList,
        JobNotDone,
        NoRewardCanClaim,
        ClaimRewardFailed,
        GetStakeParamsFailed,
        NothingToClaim,
        StakeForCommitteeFailed,
        BalanceNotEnough,
        StakeNotEnough,
        StatusNotAllowed,
        NotInNormalList,
        StatusNotFeat,
        ChangeReservedFailed,
    }
}

impl<T: Config> Pallet<T> {
    fn check_stake_health(
        who: &T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
    ) -> Result<(), ()> {
        let committee_stake_params = Self::committee_stake_params().ok_or(())?;
        let mut committee_stake = Self::committee_stake(who);

        if is_add {
            committee_stake.staked_amount =
                committee_stake.staked_amount.checked_add(&amount).ok_or(())?;
        } else {
            committee_stake.staked_amount =
                committee_stake.staked_amount.checked_sub(&amount).ok_or(())?;
        }

        ensure!(committee_stake.staked_amount >= committee_stake_params.stake_baseline, ());
        ensure!(
            committee_stake.staked_amount.saturating_sub(committee_stake.used_stake) >=
                committee_stake_params.min_free_stake_percent * committee_stake.staked_amount,
            ()
        );

        Ok(())
    }

    fn do_change_reserved(
        who: T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
        change_reserve: bool,
    ) -> Result<(), ()> {
        let mut committee_stake = Self::committee_stake(&who);

        if is_add {
            committee_stake.staked_amount =
                committee_stake.staked_amount.checked_add(&amount).ok_or(())?;
        } else {
            committee_stake.staked_amount =
                committee_stake.staked_amount.checked_sub(&amount).ok_or(())?;
        }

        if change_reserve {
            if is_add {
                ensure!(<T as Config>::Currency::can_reserve(&who, amount), ());
                <T as Config>::Currency::reserve(&who, amount).map_err(|_| ())?;
            } else {
                let _ = <T as Config>::Currency::unreserve(&who, amount);
            }
        }

        CommitteeStake::<T>::insert(&who, committee_stake);

        Ok(())
    }

    fn do_change_used_stake(
        who: T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
    ) -> Result<(), ()> {
        let mut committee_stake = Self::committee_stake(&who);
        let mut committee_list = Self::committee();

        // 计算下一阶段需要的质押数量
        committee_stake.used_stake = if is_add {
            committee_stake.used_stake.checked_add(&amount).ok_or(())?
        } else {
            committee_stake.used_stake.checked_sub(&amount).ok_or(())?
        };

        let is_committee_list_changed = Self::do_change_status_when_stake_changed(
            who.clone(),
            &mut committee_list,
            &committee_stake,
        );

        if is_committee_list_changed {
            Committee::<T>::put(committee_list);
        }
        CommitteeStake::<T>::insert(&who, committee_stake);

        Ok(())
    }

    // 根据当前质押量，修改committee状态
    fn do_change_status_when_stake_changed(
        committee: T::AccountId,
        committee_list: &mut CommitteeList<T::AccountId>,
        committee_stake: &CommitteeStakeInfo<BalanceOf<T>>,
    ) -> bool {
        let committee_stake_params = Self::committee_stake_params().unwrap_or_default();
        let is_free_stake_enough = committee_stake.staked_amount >=
            committee_stake_params.stake_baseline &&
            committee_stake.staked_amount.saturating_sub(committee_stake.used_stake) >=
                committee_stake_params.min_free_stake_percent * committee_stake.staked_amount;
        let mut is_status_changed = false;

        if is_free_stake_enough && committee_list.is_fulfilling(&committee) {
            ItemList::rm_item(&mut committee_list.fulfilling_list, &committee);
            ItemList::add_item(&mut committee_list.normal, committee);
            is_status_changed = true;
        } else if !is_free_stake_enough && committee_list.is_normal(&committee) {
            ItemList::rm_item(&mut committee_list.normal, &committee);
            ItemList::add_item(&mut committee_list.fulfilling_list, committee);
            is_status_changed = true;
        }

        is_status_changed
    }
}
