#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
    ensure,
    pallet_prelude::*,
    traits::{BalanceStatus, Currency, EnsureOrigin, OnUnbalanced, ReservableCurrency},
    weights::Weight,
    IterableStorageMap,
};
use frame_system::pallet_prelude::*;
use generic_func::ItemList;
use online_profile_machine::ManageCommittee;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
    traits::{CheckedAdd, CheckedSub, SaturatedConversion, Zero},
    Perbill, RuntimeDebug,
};
use sp_std::{collections::btree_set::BTreeSet, prelude::*, str, vec::Vec};

pub type SlashId = u64;
type BalanceOf<T> = <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

// 即将被执行的罚款
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CMPendingSlashInfo<AccountId, BlockNumber, Balance> {
    /// 被惩罚人
    pub slash_who: AccountId,
    /// 惩罚被创建的时间
    pub slash_time: BlockNumber,
    /// 执行惩罚的金额
    pub slash_amount: Balance,
    /// 惩罚被执行的时间
    pub slash_exec_time: BlockNumber,
    /// 奖励发放对象。如果为空，则惩罚到国库
    pub reward_to: Vec<AccountId>,
    /// 委员会被惩罚的原因
    pub slash_reason: CMSlashReason,
}

// TODO: 在OC中，惩罚全部都是NotSubmitRaw，可能需要更精细区分
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum CMSlashReason {
    OCNotSubmitHash,
    OCNotSubmitRaw,
    OCInconsistentSubmit,
    MCNotSubmitHash,
    MCNotSubmitRaw,
    MCInconsistentSubmit,
}

impl Default for CMSlashReason {
    fn default() -> Self {
        Self::OCNotSubmitHash
    }
}

// 处于不同状态的委员会的列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct CommitteeList<AccountId: Ord> {
    /// 质押并通过社区选举的委员会，正常状态
    pub normal: Vec<AccountId>,
    /// 委员会，但不想被派单
    pub chill_list: Vec<AccountId>,
    /// 等待提交box pubkey的委员会
    pub waiting_box_pubkey: Vec<AccountId>,
    /// 等待补充质押的委员会
    pub fulfilling_list: Vec<AccountId>,
}

impl<AccountId: Ord> CommitteeList<AccountId> {
    fn is_in_committee(&self, who: &AccountId) -> bool {
        self.normal.binary_search(who).is_ok() ||
            self.chill_list.binary_search(who).is_ok() ||
            self.waiting_box_pubkey.binary_search(who).is_ok() ||
            self.fulfilling_list.binary_search(who).is_ok()
    }
}

/// 与委员会质押基本参数
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeStakeParamsInfo<Balance> {
    /// 第一次委员会质押的基准数值
    pub stake_baseline: Balance,
    /// 每次订单使用的质押数量 & apply_slash_review stake amount
    pub stake_per_order: Balance,
    /// 当剩余的质押数量到阈值时，需要补质押
    pub min_free_stake_percent: Perbill,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CMPendingSlashReviewInfo<AccountId, Balance, BlockNumber> {
    pub applicant: AccountId,
    pub staked_amount: Balance,
    pub apply_time: BlockNumber,
    pub expire_time: BlockNumber,
}

/// 委员会质押的状况
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeStakeInfo<Balance> {
    pub box_pubkey: [u8; 32],
    pub staked_amount: Balance,
    pub used_stake: Balance,
    pub can_claim_reward: Balance,
    pub claimed_reward: Balance,
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
        type CancelSlashOrigin: EnsureOrigin<Self::Origin>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> Weight {
            0
        }

        fn on_initialize(_n: BlockNumberFor<T>) -> frame_support::weights::Weight {
            let _ = Self::check_and_exec_slash();
            0
        }

        fn on_finalize(_n: BlockNumberFor<T>) {
            let _ = Self::check_and_exec_review();
        }
    }

    #[pallet::storage]
    #[pallet::getter(fn next_slash_id)]
    pub(super) type NextSlashId<T: Config> = StorageValue<_, SlashId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pending_slash)]
    pub(super) type PendingSlash<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        SlashId,
        CMPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn pending_slash_review)]
    pub(super) type PendingSlashReview<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        SlashId,
        CMPendingSlashReviewInfo<T::AccountId, BalanceOf<T>, T::BlockNumber>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn committee)]
    pub(super) type Committee<T: Config> = StorageValue<_, CommitteeList<T::AccountId>, ValueQuery>;

    /// 委员会质押模块基本参数
    #[pallet::storage]
    #[pallet::getter(fn committee_stake_params)]
    pub(super) type CommitteeStakeParams<T: Config> = StorageValue<_, CommitteeStakeParamsInfo<BalanceOf<T>>>;

    /// 委员会质押与收益情况
    #[pallet::storage]
    #[pallet::getter(fn committee_stake)]
    pub(super) type CommitteeStake<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, CommitteeStakeInfo<BalanceOf<T>>, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置committee每次操作需要质押数量
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
        // 添加到委员会，直接添加到fulfill列表中。每次finalize将会读取委员会币数量，币足则放到committee中
        #[pallet::weight(0)]
        pub fn add_committee(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut committee = Self::committee();
            // 确保用户还未加入到本模块
            ensure!(!committee.is_in_committee(&member), Error::<T>::AccountAlreadyExist);
            // 将用户添加到waiting_box_pubkey列表中
            ItemList::add_item(&mut committee.waiting_box_pubkey, member.clone());

            Committee::<T>::put(committee);
            Self::deposit_event(Event::CommitteeAdded(member));
            Ok(().into())
        }

        /// 委员会添用于非对称加密的公钥信息，并绑定质押
        #[pallet::weight(10000)]
        pub fn committee_set_box_pubkey(origin: OriginFor<T>, box_pubkey: [u8; 32]) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let mut committee_list = Self::committee();
            let mut committee_stake = Self::committee_stake(&committee);
            let committee_stake_params = Self::committee_stake_params().ok_or(Error::<T>::GetStakeParamsFailed)?;

            if committee_list.normal.binary_search(&committee).is_ok() {
                committee_stake.box_pubkey = box_pubkey;
                CommitteeStake::<T>::insert(&committee, committee_stake);
                Self::deposit_event(Event::CommitteeSetBoxPubkey(committee, box_pubkey));
                return Ok(().into())
            }

            // 只允许委员会第一次操作
            ensure!(committee_list.waiting_box_pubkey.binary_search(&committee).is_ok(), Error::<T>::NotCommittee);
            ensure!(
                <T as Config>::Currency::can_reserve(&committee, committee_stake_params.stake_baseline),
                Error::<T>::BalanceNotEnough
            );

            <T as pallet::Config>::Currency::reserve(&committee, committee_stake_params.stake_baseline)
                .map_err(|_| Error::<T>::GetStakeParamsFailed)?;

            committee_stake.box_pubkey = box_pubkey;
            committee_stake.staked_amount = committee_stake_params.stake_baseline;

            ItemList::rm_item(&mut committee_list.waiting_box_pubkey, &committee);
            ItemList::add_item(&mut committee_list.normal, committee.clone());

            Committee::<T>::put(committee_list);
            CommitteeStake::<T>::insert(&committee, committee_stake);

            Self::deposit_event(Event::StakeAdded(committee.clone(), committee_stake_params.stake_baseline));
            Self::deposit_event(Event::CommitteeSetBoxPubkey(committee, box_pubkey));
            Ok(().into())
        }

        /// 委员会增加质押
        #[pallet::weight(10000)]
        pub fn committee_add_stake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let mut committee_stake = Self::committee_stake(&committee);
            let mut committee_list = Self::committee();
            let committee_stake_params = Self::committee_stake_params().ok_or(Error::<T>::GetStakeParamsFailed)?;

            ensure!(committee_list.is_in_committee(&committee), Error::<T>::NotCommittee);

            committee_stake.staked_amount =
                committee_stake.staked_amount.checked_add(&amount).ok_or(Error::<T>::StakeNotEnough)?;
            // 保证新增加质押之后，用户质押量需要大于基本质押
            ensure!(committee_stake.staked_amount > committee_stake_params.stake_baseline, Error::<T>::StakeNotEnough);
            ensure!(
                committee_stake.staked_amount - committee_stake.used_stake >
                    committee_stake_params.min_free_stake_percent * committee_stake.staked_amount,
                Error::<T>::StakeNotEnough
            );
            ensure!(
                <T as Config>::Currency::can_reserve(&committee, committee_stake_params.stake_baseline),
                Error::<T>::BalanceNotEnough
            );

            <T as pallet::Config>::Currency::reserve(&committee, committee_stake_params.stake_baseline)
                .map_err(|_| Error::<T>::GetStakeParamsFailed)?;

            if committee_list.fulfilling_list.binary_search(&committee).is_ok() {
                ItemList::rm_item(&mut committee_list.fulfilling_list, &committee);
                ItemList::add_item(&mut committee_list.fulfilling_list, committee.clone());
                Committee::<T>::put(committee_list);
            }

            CommitteeStake::<T>::insert(&committee, committee_stake);
            Self::deposit_event(Event::StakeAdded(committee, amount));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn committee_reduce_stake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let mut committee_stake = Self::committee_stake(&committee);
            let committee_list = Self::committee();
            let committee_stake_params = Self::committee_stake_params().ok_or(Error::<T>::GetStakeParamsFailed)?;

            // ensure!(committee_list.is_in_committee(&committee), lrror::<T>::NotCommittee);
            ensure!(committee_list.normal.binary_search(&committee).is_ok(), Error::<T>::NotInNormalList);

            committee_stake.staked_amount =
                committee_stake.staked_amount.checked_sub(&amount).ok_or(Error::<T>::BalanceNotEnough)?;

            ensure!(
                committee_stake.staked_amount >= committee_stake_params.stake_baseline,
                Error::<T>::BalanceNotEnough
            );

            let left_free_amount = committee_stake
                .staked_amount
                .checked_sub(&committee_stake.used_stake)
                .ok_or(Error::<T>::BalanceNotEnough)?;

            ensure!(
                committee_stake_params.min_free_stake_percent * committee_stake.staked_amount >= left_free_amount,
                Error::<T>::BalanceNotEnough
            );

            let _ = <T as pallet::Config>::Currency::unreserve(&committee, amount);

            CommitteeStake::<T>::insert(&committee, committee_stake);
            Self::deposit_event(Event::StakeReduced(committee, amount));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn claim_reward(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;

            let mut committee_stake = Self::committee_stake(&committee);
            ensure!(committee_stake.can_claim_reward != Zero::zero(), Error::<T>::NothingToClaim);

            let can_claim_reward = committee_stake.can_claim_reward;
            committee_stake.claimed_reward += can_claim_reward;
            committee_stake.can_claim_reward = Zero::zero();

            <T as pallet::Config>::Currency::deposit_into_existing(&committee, can_claim_reward)
                .map_err(|_| Error::<T>::ClaimRewardFailed)?;

            CommitteeStake::<T>::insert(&committee, committee_stake);
            Self::deposit_event(Event::ClaimReward(committee, can_claim_reward));
            Ok(().into())
        }

        // 委员会停止接单
        #[pallet::weight(10000)]
        pub fn chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let mut committee_list = Self::committee();

            if committee_list.fulfilling_list.binary_search(&committee).is_ok() {
                return Ok(().into())
            }

            ensure!(committee_list.is_in_committee(&committee), Error::<T>::NotCommittee);
            // waiting_box_pubkey不能执行该操作
            ensure!(committee_list.waiting_box_pubkey.binary_search(&committee).is_err(), Error::<T>::PubkeyNotSet);

            // Allow normal & fulfilling committee to chill
            ItemList::rm_item(&mut committee_list.normal, &committee);
            ItemList::rm_item(&mut committee_list.fulfilling_list, &committee);
            ItemList::add_item(&mut committee_list.chill_list, committee.clone());

            Committee::<T>::put(committee_list);
            Self::deposit_event(Event::Chill(committee));
            Ok(().into())
        }

        // 委员会可以接单
        #[pallet::weight(10000)]
        pub fn undo_chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let mut committee_list = Self::committee();

            ensure!(committee_list.chill_list.binary_search(&committee).is_ok(), Error::<T>::NotInChillList);

            ItemList::rm_item(&mut committee_list.chill_list, &committee);
            ItemList::add_item(&mut committee_list.normal, committee.clone());

            let _ = Self::change_committee_status_when_stake_changed(
                committee.clone(),
                &mut committee_list,
                &Self::committee_stake(&committee),
            );

            Committee::<T>::put(committee_list);
            Self::deposit_event(Event::UndoChill(committee));
            Ok(().into())
        }

        /// Only In Chill list & used_stake == 0 can exit.
        #[pallet::weight(10000)]
        pub fn exit_committee(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let mut committee_stake = Self::committee_stake(&committee);

            let mut committee_list = Self::committee();
            ensure!(committee_stake.used_stake == Zero::zero(), Error::<T>::JobNotDone);
            ensure!(committee_list.chill_list.binary_search(&committee).is_ok(), Error::<T>::StatusNotFeat);

            ItemList::rm_item(&mut committee_list.chill_list, &committee);
            let _ = <T as pallet::Config>::Currency::unreserve(&committee, committee_stake.staked_amount);

            committee_stake.staked_amount = Zero::zero();

            CommitteeStake::<T>::insert(&committee, committee_stake);
            Committee::<T>::put(committee_list);
            Self::deposit_event(Event::ExitFromCandidacy(committee));
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn apply_slash_review(origin: OriginFor<T>, slash_id: SlashId) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;

            let now = <frame_system::Module<T>>::block_number();
            let committee_stake_params = Self::committee_stake_params().ok_or(Error::<T>::GetStakeParamsFailed)?;
            let mut committee_stake = Self::committee_stake(&committee);

            let slash_info = Self::pending_slash(slash_id);
            ensure!(slash_info.slash_who == committee, Error::<T>::NotSlashed);
            ensure!(slash_info.slash_exec_time > now, Error::<T>::ExpiredSlash);

            committee_stake.staked_amount = committee_stake
                .staked_amount
                .checked_sub(&committee_stake_params.stake_per_order)
                .ok_or(Error::<T>::BalanceNotEnough)?;
            ensure!(
                committee_stake.staked_amount - committee_stake.used_stake >
                    committee_stake_params.min_free_stake_percent * committee_stake.staked_amount,
                Error::<T>::StakeNotEnough
            );

            CommitteeStake::<T>::insert(&committee, committee_stake);
            PendingSlashReview::<T>::insert(
                slash_id,
                CMPendingSlashReviewInfo {
                    applicant: committee.clone(),
                    staked_amount: committee_stake_params.stake_per_order,
                    apply_time: now,
                    expire_time: slash_info.slash_exec_time,
                },
            );
            Self::deposit_event(Event::StakeAdded(committee, committee_stake_params.stake_per_order));
            Self::deposit_event(Event::ApplySlashReview(slash_id));
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn cancel_slash(origin: OriginFor<T>, slash_id: SlashId) -> DispatchResultWithPostInfo {
            T::CancelSlashOrigin::ensure_origin(origin)?;
            ensure!(PendingSlash::<T>::contains_key(slash_id), Error::<T>::SlashIDNotExist);
            ensure!(PendingSlashReview::<T>::contains_key(slash_id), Error::<T>::NotPendingReviewSlash);

            let slash_info = Self::pending_slash(slash_id);
            let slash_review_info = Self::pending_slash_review(slash_id);
            let mut committee_stake = Self::committee_stake(&slash_info.slash_who);
            let mut committee_list = Self::committee();

            committee_stake.used_stake = committee_stake
                .used_stake
                .checked_sub(&slash_info.slash_amount)
                .ok_or(Error::<T>::CancelSlashFailed)?
                .checked_sub(&slash_review_info.staked_amount)
                .ok_or(Error::<T>::CancelSlashFailed)?;

            let is_committee_list_changed = Self::change_committee_status_when_stake_changed(
                slash_info.slash_who.clone(),
                &mut committee_list,
                &committee_stake,
            );

            let _ = <T as pallet::Config>::Currency::unreserve(
                &slash_info.slash_who,
                slash_info.slash_amount + slash_review_info.staked_amount,
            );

            CommitteeStake::<T>::insert(&slash_info.slash_who, committee_stake);
            if is_committee_list_changed {
                Committee::<T>::put(committee_list);
            }

            // TODO: should slash reward_to to origin slashd one

            PendingSlash::<T>::remove(slash_id);
            PendingSlashReview::<T>::remove(slash_id);
            Self::deposit_event(Event::StakeReduced(slash_info.slash_who.clone(), slash_info.slash_amount));
            Self::deposit_event(Event::SlashCanceled(slash_id, slash_info.slash_who, slash_info.slash_amount));
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        PayTxFee(T::AccountId, BalanceOf<T>),
        CommitteeAdded(T::AccountId),
        CommitteeFulfill(BalanceOf<T>),
        Chill(T::AccountId),
        CommitteeExit(T::AccountId),
        UndoChill(T::AccountId),
        Slash(T::AccountId, BalanceOf<T>),
        MissedSlash(T::AccountId, BalanceOf<T>),
        ExitFromCandidacy(T::AccountId),
        CommitteeSetBoxPubkey(T::AccountId, [u8; 32]),
        StakeAdded(T::AccountId, BalanceOf<T>),
        StakeReduced(T::AccountId, BalanceOf<T>),
        ClaimReward(T::AccountId, BalanceOf<T>),
        SlashCanceled(u64, T::AccountId, BalanceOf<T>),
        ApplySlashReview(SlashId),
        SlashReviewFailed(SlashId, T::AccountId, BalanceOf<T>),
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
        CancelSlashFailed,
        SlashIDNotExist,
        StatusNotFeat,
        NotSlashed,
        ExpiredSlash,
        NotPendingReviewSlash,
    }
}

impl<T: Config> Pallet<T> {
    // 检查并执行slash
    // TODO: after slash is done, should unreserve balance of committee,
    fn check_and_exec_slash() -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let pending_slash_id = Self::get_slash_id();

        for slash_id in pending_slash_id {
            let slash_info = Self::pending_slash(&slash_id);
            if now >= slash_info.slash_exec_time {
                // 如果reward_to为0，则将币转到国库
                let reward_to_num = slash_info.reward_to.len() as u32;

                let mut committee_stake = Self::committee_stake(&slash_info.slash_who);
                let mut committee_list = Self::committee();

                committee_stake.used_stake =
                    committee_stake.used_stake.checked_sub(&slash_info.slash_amount).ok_or(())?;
                committee_stake.staked_amount =
                    committee_stake.staked_amount.checked_sub(&slash_info.slash_amount).ok_or(())?;

                let is_committee_list_changed = Self::change_committee_status_when_stake_changed(
                    slash_info.slash_who.clone(),
                    &mut committee_list,
                    &committee_stake,
                );

                if reward_to_num == 0 {
                    if <T as pallet::Config>::Currency::reserved_balance(&slash_info.slash_who) >=
                        slash_info.slash_amount
                    {
                        let (imbalance, _missing) = <T as pallet::Config>::Currency::slash_reserved(
                            &slash_info.slash_who,
                            slash_info.slash_amount,
                        );
                        <T as pallet::Config>::Slash::on_unbalanced(imbalance);

                        PendingSlash::<T>::remove(slash_id);
                    }
                } else {
                    let reward_each_get =
                        Perbill::from_rational_approximation(1u32, reward_to_num) * slash_info.slash_amount;
                    let mut left_reward = slash_info.slash_amount;

                    for a_committee in slash_info.reward_to {
                        if <T as pallet::Config>::Currency::reserved_balance(&slash_info.slash_who) >= left_reward {
                            if left_reward >= reward_each_get {
                                let _ = <T as pallet::Config>::Currency::repatriate_reserved(
                                    &slash_info.slash_who,
                                    &a_committee,
                                    reward_each_get,
                                    BalanceStatus::Free,
                                );
                                left_reward = left_reward.checked_sub(&reward_each_get).ok_or(())?;
                            } else {
                                let _ = <T as pallet::Config>::Currency::repatriate_reserved(
                                    &slash_info.slash_who,
                                    &a_committee,
                                    left_reward,
                                    BalanceStatus::Free,
                                );
                            }
                        }
                    }
                }

                if is_committee_list_changed {
                    Committee::<T>::put(committee_list);
                }
                CommitteeStake::<T>::insert(&slash_info.slash_who, committee_stake);
                PendingSlash::<T>::remove(slash_id);
            }
        }
        Ok(())
    }

    fn check_and_exec_review() -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let all_review_id = <PendingSlashReview<T> as IterableStorageMap<SlashId, _>>::iter()
            .map(|(slash_id, _)| slash_id)
            .collect::<BTreeSet<_>>();

        for a_id in all_review_id {
            let review_info = Self::pending_slash_review(a_id);
            ensure!(now >= review_info.expire_time, ());

            let mut committee_stake = Self::committee_stake(&review_info.applicant);
            let mut committee_list = Self::committee();

            committee_stake.used_stake =
                committee_stake.used_stake.checked_sub(&review_info.staked_amount).unwrap_or_default();
            committee_stake.staked_amount =
                committee_stake.staked_amount.checked_sub(&review_info.staked_amount).unwrap_or_default();

            let is_committee_list_changed = Self::change_committee_status_when_stake_changed(
                review_info.applicant.clone(),
                &mut committee_list,
                &committee_stake,
            );

            // reserved to treasury and change committee total_stake & used stake
            if <T as pallet::Config>::Currency::reserved_balance(&review_info.applicant) >= review_info.staked_amount {
                let (imbalance, _missing) =
                    <T as pallet::Config>::Currency::slash_reserved(&review_info.applicant, review_info.staked_amount);
                <T as pallet::Config>::Slash::on_unbalanced(imbalance);
            }

            if is_committee_list_changed {
                Committee::<T>::put(committee_list);
            }
            PendingSlashReview::<T>::remove(a_id);
            Self::deposit_event(Event::SlashReviewFailed(a_id, review_info.applicant, review_info.staked_amount));
        }

        Ok(())
    }

    // 获得所有被惩罚的订单列表
    fn get_slash_id() -> BTreeSet<SlashId> {
        <PendingSlash<T> as IterableStorageMap<SlashId, _>>::iter()
            .map(|(slash_id, _)| slash_id)
            .collect::<BTreeSet<_>>()
    }

    fn get_new_slash_id() -> SlashId {
        let slash_id = Self::next_slash_id();
        if slash_id == u64::MAX {
            NextSlashId::<T>::put(0);
        } else {
            NextSlashId::<T>::put(slash_id + 1);
        };
        slash_id
    }

    // 根据当前质押量，修改committee状态
    fn change_committee_status_when_stake_changed(
        committee: T::AccountId,
        committee_list: &mut CommitteeList<T::AccountId>,
        committee_stake: &CommitteeStakeInfo<BalanceOf<T>>,
    ) -> bool {
        let committee_stake_params = Self::committee_stake_params().unwrap_or_default();
        let is_free_stake_enough = committee_stake.staked_amount - committee_stake.used_stake >=
            committee_stake_params.min_free_stake_percent * committee_stake.staked_amount;
        let mut is_committee_list_changed = false;

        if is_free_stake_enough && committee_list.fulfilling_list.binary_search(&committee).is_ok() {
            ItemList::rm_item(&mut committee_list.fulfilling_list, &committee);
            ItemList::add_item(&mut committee_list.normal, committee);
            is_committee_list_changed = true;
        } else if committee_list.normal.binary_search(&committee).is_ok() {
            ItemList::rm_item(&mut committee_list.normal, &committee);
            ItemList::add_item(&mut committee_list.fulfilling_list, committee);
            is_committee_list_changed = true;
        }

        is_committee_list_changed
    }
}

impl<T: Config> ManageCommittee for Pallet<T> {
    type AccountId = T::AccountId;
    type BalanceOf = BalanceOf<T>;
    type SlashReason = CMSlashReason;

    // 检查是否为状态正常的委员会
    fn is_valid_committee(who: &T::AccountId) -> bool {
        Self::committee().normal.binary_search(&who).is_ok()
    }

    // 检查委员会是否有足够的质押,返回有可以抢单的机器列表
    // 在每个区块以及每次分配一个机器之后，都需要检查
    fn available_committee() -> Option<Vec<T::AccountId>> {
        let committee_list = Self::committee();
        let normal_committee = committee_list.normal.clone();
        let stake_params = Self::committee_stake_params()?;
        let mut out = Vec::new();

        // 如果free_balance足够，则复制到out列表中
        for a_committee in normal_committee {
            // 当委员会质押不够时，将委员会移动到fulfill_list中
            if <T as Config>::Currency::free_balance(&a_committee) > stake_params.stake_per_order {
                out.push(a_committee.clone());
            }
        }

        (out.len() > 0).then(|| out)
    }

    // 改变委员会使用的质押数量
    // - Writes: CommitteeStake, Committee
    fn change_used_stake(committee: T::AccountId, amount: BalanceOf<T>, is_add: bool) -> Result<(), ()> {
        let mut committee_stake = Self::committee_stake(&committee);
        let mut committee_list = Self::committee();

        // 计算下一阶段需要的质押数量
        committee_stake.used_stake = if is_add {
            committee_stake.used_stake.checked_add(&amount).ok_or(())?
        } else {
            committee_stake.used_stake.checked_sub(&amount).ok_or(())?
        };

        let is_committee_list_changed =
            Self::change_committee_status_when_stake_changed(committee.clone(), &mut committee_list, &committee_stake);

        if is_committee_list_changed {
            Committee::<T>::put(committee_list);
        }
        CommitteeStake::<T>::insert(&committee, committee_stake);

        Ok(())
    }

    fn stake_per_order() -> Option<BalanceOf<T>> {
        Some(Self::committee_stake_params()?.stake_per_order)
    }

    fn add_reward(committee: T::AccountId, reward: BalanceOf<T>) {
        let mut committee_stake = Self::committee_stake(&committee);
        committee_stake.can_claim_reward += reward;
        CommitteeStake::<T>::insert(&committee, committee_stake);
    }

    fn add_slash(who: T::AccountId, amount: BalanceOf<T>, reward_to: Vec<T::AccountId>, slash_reason: CMSlashReason) {
        let slash_id = Self::get_new_slash_id();
        let now = <frame_system::Module<T>>::block_number();
        PendingSlash::<T>::insert(
            slash_id,
            CMPendingSlashInfo {
                slash_who: who,
                slash_time: now,
                slash_amount: amount,
                slash_exec_time: now + 5760u32.saturated_into::<T::BlockNumber>(),
                reward_to,
                slash_reason,
            },
        );
    }
}

// RPC
impl<T: Config> Module<T> {
    pub fn get_committee_list() -> CommitteeList<T::AccountId> {
        Self::committee()
    }
}
