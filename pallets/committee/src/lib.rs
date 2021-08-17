#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
    ensure,
    pallet_prelude::*,
    traits::{
        Currency, ExistenceRequirement::KeepAlive, LockIdentifier, LockableCurrency, OnUnbalanced, WithdrawReasons,
    },
    IterableStorageMap,
};
use frame_system::pallet_prelude::*;
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

pub const PALLET_LOCK_ID: LockIdentifier = *b"committe";

// 即将被执行的罚款
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct PendingSlashInfo<AccountId, BlockNumber, Balance> {
    /// 被惩罚人
    pub slash_who: AccountId,
    /// 惩罚被创建的时间
    pub slash_time: BlockNumber,
    /// 执行惩罚前解绑的金额
    pub unlock_amount: Balance,
    /// 执行惩罚的金额
    pub slash_amount: Balance,
    /// 惩罚被执行的时间
    pub slash_exec_time: BlockNumber,
    /// 奖励发放对象。如果为空，则惩罚到国库
    pub reward_to: Vec<AccountId>,
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

/// 与委员会质押基本参数
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeStakeParamsInfo<Balance> {
    /// 第一次委员会质押的基准数值
    pub stake_baseline: Balance,
    /// 每次订单使用的质押数量
    pub stake_per_order: Balance,
    /// 当剩余的质押数量到阈值时，需要补质押
    pub min_free_stake_percent: Perbill,
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

impl<AccountId: Ord> CommitteeList<AccountId> {
    fn is_in_committee(&self, who: &AccountId) -> bool {
        if self.normal.binary_search(who).is_ok() ||
            self.chill_list.binary_search(who).is_ok() ||
            self.waiting_box_pubkey.binary_search(who).is_ok() ||
            self.fulfilling_list.binary_search(who).is_ok()
        {
            return true
        }
        false
    }

    fn add_one(a_field: &mut Vec<AccountId>, who: AccountId) {
        if let Err(index) = a_field.binary_search(&who) {
            a_field.insert(index, who);
        }
    }

    fn rm_one(a_field: &mut Vec<AccountId>, who: &AccountId) {
        if let Ok(index) = a_field.binary_search(who) {
            a_field.remove(index);
        }
    }
}

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_block_number: T::BlockNumber) {
            // Self::check_and_exec_slash();
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
        PendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
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
            CommitteeList::add_one(&mut committee.waiting_box_pubkey, member.clone());

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

            // 确保是委员会才能执行该操作
            ensure!(committee_list.is_in_committee(&committee), Error::<T>::NotCommittee);
            // 检查free_balance
            ensure!(
                <T as Config>::Currency::free_balance(&committee) > committee_stake_params.stake_per_order,
                Error::<T>::BalanceNotEnough
            );

            <T as Config>::Currency::set_lock(
                PALLET_LOCK_ID,
                &committee,
                committee_stake_params.stake_baseline,
                WithdrawReasons::all(),
            );

            committee_stake.box_pubkey = box_pubkey;
            committee_stake.staked_amount = committee_stake_params.stake_baseline;

            if committee_list.waiting_box_pubkey.binary_search(&committee).is_ok() {
                CommitteeList::rm_one(&mut committee_list.waiting_box_pubkey, &committee);
                CommitteeList::add_one(&mut committee_list.normal, committee.clone());
                Committee::<T>::put(committee_list);
            }

            CommitteeStake::<T>::insert(committee, committee_stake);

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

            // 检查free_balance
            ensure!(
                <T as Config>::Currency::free_balance(&committee) > committee_stake_params.stake_per_order,
                Error::<T>::BalanceNotEnough
            );

            <T as Config>::Currency::set_lock(
                PALLET_LOCK_ID,
                &committee,
                committee_stake.staked_amount,
                WithdrawReasons::all(),
            );

            if let Ok(index) = committee_list.fulfilling_list.binary_search(&committee) {
                committee_list.fulfilling_list.remove(index);
                if let Err(index) = committee_list.normal.binary_search(&committee) {
                    committee_list.normal.insert(index, committee.clone());
                    Committee::<T>::put(committee_list);
                }
            }

            CommitteeStake::<T>::insert(&committee, committee_stake);

            Ok(().into())
        }

        /// 状态正常的委员会更改加密pubkey
        #[pallet::weight(10000)]
        pub fn committee_change_pubkey(origin: OriginFor<T>, box_pubkey: [u8; 32]) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let mut committee_stake = Self::committee_stake(&committee);
            let committee_list = Self::committee();

            ensure!(committee_list.normal.binary_search(&committee).is_ok(), Error::<T>::StatusNotAllowed);

            committee_stake.box_pubkey = box_pubkey;
            CommitteeStake::<T>::insert(&committee, committee_stake);
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn claim_reward(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;

            let mut committee_stake = Self::committee_stake(&committee);
            ensure!(committee_stake.can_claim_reward != Zero::zero(), Error::<T>::NothingToClaim);

            <T as pallet::Config>::Currency::deposit_into_existing(&committee, committee_stake.can_claim_reward)
                .map_err(|_| Error::<T>::ClaimRewardFailed)?;

            committee_stake.claimed_reward += committee_stake.can_claim_reward;
            committee_stake.can_claim_reward = Zero::zero();

            CommitteeStake::<T>::insert(&committee, committee_stake);

            Ok(().into())
        }

        // 委员会停止接单
        #[pallet::weight(10000)]
        pub fn chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;

            let mut committee_list = Self::committee();
            ensure!(committee_list.is_in_committee(&committee), Error::<T>::NotCommittee);

            // waiting_box_pubkey不能执行该操作
            if committee_list.waiting_box_pubkey.binary_search(&committee).is_ok() {
                return Err(Error::<T>::PubkeyNotSet.into())
            }

            CommitteeList::rm_one(&mut committee_list.normal, &committee);
            CommitteeList::add_one(&mut committee_list.chill_list, committee.clone());

            Committee::<T>::put(committee_list);
            Self::deposit_event(Event::Chill(committee));

            Ok(().into())
        }

        // 委员会可以接单
        #[pallet::weight(10000)]
        pub fn undo_chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut committee_list = Self::committee();
            committee_list.chill_list.binary_search(&who).map_err(|_| Error::<T>::NotInChillList)?;

            CommitteeList::rm_one(&mut committee_list.chill_list, &who);
            Committee::<T>::put(committee_list);

            Self::deposit_event(Event::UndoChill(who));
            Ok(().into())
        }

        // 委员会可以退出, 从chill_list中退出
        // 只有当委员会质押为0时才能退出，此时委员会没有待处理任务
        #[pallet::weight(10000)]
        pub fn exit_staker(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let mut committee_stake = Self::committee_stake(&committee);

            let mut committee_list = Self::committee();
            ensure!(committee_list.is_in_committee(&committee), Error::<T>::NotCommittee);
            ensure!(committee_stake.used_stake == Zero::zero(), Error::<T>::JobNotDone);

            // TODO: change stake
            CommitteeList::rm_one(&mut committee_list.normal, &committee);
            CommitteeList::rm_one(&mut committee_list.chill_list, &committee);
            CommitteeList::rm_one(&mut committee_list.waiting_box_pubkey, &committee);

            Committee::<T>::put(committee_list);

            Self::deposit_event(Event::ExitFromCandidacy(committee));

            return Ok(().into())
        }

        // 取消一个惩罚
        // FIXME: 应该将锁定的币直接返还
        #[pallet::weight(0)]
        pub fn cancle_slash(origin: OriginFor<T>, slash_id: SlashId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            PendingSlash::<T>::remove(slash_id);
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
    }
}

impl<T: Config> Pallet<T> {
    // // 检查并执行slash
    // fn check_and_exec_slash() {
    //     let now = <frame_system::Module<T>>::block_number();

    //     let pending_slash_id = Self::get_slash_id();
    //     for a_slash_id in pending_slash_id {
    //         let a_slash_info = Self::pending_slash(&a_slash_id);
    //         if now >= a_slash_info.slash_exec_time {
    //             let _ = Self::change_total_stake(&a_slash_info.slash_who, a_slash_info.unlock_amount, false);

    //             // 如果reward_to为0，则将币转到国库
    //             let reward_to_num = a_slash_info.reward_to.len() as u32;
    //             if reward_to_num == 0 {
    //                 if <T as pallet::Config>::Currency::can_slash(&a_slash_info.slash_who, a_slash_info.slash_amount) {
    //                     let (imbalance, missing) =
    //                         <T as pallet::Config>::Currency::slash(&a_slash_info.slash_who, a_slash_info.slash_amount);
    //                     Self::deposit_event(Event::Slash(a_slash_info.slash_who.clone(), a_slash_info.slash_amount));
    //                     Self::deposit_event(Event::MissedSlash(a_slash_info.slash_who, missing.clone()));
    //                     <T as pallet::Config>::Slash::on_unbalanced(imbalance);
    //                 }
    //             } else {
    //                 let reward_each_get =
    //                     Perbill::from_rational_approximation(1u32, reward_to_num) * a_slash_info.slash_amount;

    //                 let mut left_reward = a_slash_info.slash_amount;

    //                 for a_committee in a_slash_info.reward_to {
    //                     if left_reward < reward_each_get {
    //                         if <T as pallet::Config>::Currency::transfer(
    //                             &a_slash_info.slash_who,
    //                             &a_committee,
    //                             left_reward,
    //                             KeepAlive,
    //                         )
    //                         .is_err()
    //                         {
    //                             debug::error!("Left reward is less than reward each get");
    //                         }
    //                         return
    //                     } else {
    //                         if <T as pallet::Config>::Currency::transfer(
    //                             &a_slash_info.slash_who,
    //                             &a_committee,
    //                             reward_each_get,
    //                             KeepAlive,
    //                         )
    //                         .is_err()
    //                         {
    //                             debug::error!("Transfer slash to reward_to failed");
    //                         }
    //                         left_reward -= reward_each_get;
    //                     }
    //                 }
    //             }
    //         }
    //     }
    // }

    // 获得所有被惩罚的订单列表
    fn get_slash_id() -> BTreeSet<SlashId> {
        <PendingSlash<T> as IterableStorageMap<SlashId, _>>::iter()
            .map(|(slash_id, _)| slash_id)
            .collect::<BTreeSet<_>>()
    }

    fn get_new_slash_id() -> SlashId {
        let slash_id = Self::next_slash_id();
        NextSlashId::<T>::put(slash_id + 1);
        return slash_id
    }
}

impl<T: Config> ManageCommittee for Pallet<T> {
    type AccountId = T::AccountId;
    type BalanceOf = BalanceOf<T>;

    // 检查是否为状态正常的委员会
    fn is_valid_committee(who: &T::AccountId) -> bool {
        let committee_list = Self::committee();
        committee_list.normal.binary_search(&who).is_ok()
    }

    // 检查委员会是否有足够的质押,返回有可以抢单的机器列表
    // 在每个区块以及每次分配一个机器之后，都需要检查
    fn available_committee() -> Result<Vec<T::AccountId>, ()> {
        let committee_list = Self::committee();
        let stake_params = Self::committee_stake_params().ok_or(())?;

        let normal_committee = committee_list.normal.clone();

        let mut out = Vec::new();

        // 如果free_balance足够，则复制到out列表中
        for a_committee in normal_committee {
            // 当委员会质押不够时，将委员会移动到fulfill_list中
            if <T as Config>::Currency::free_balance(&a_committee) > stake_params.stake_per_order {
                out.push(a_committee.clone());
            }
        }

        if out.len() > 0 {
            return Ok(out)
        }
        return Err(())
    }

    // 改变委员会使用的质押数量
    fn change_used_stake(committee: &T::AccountId, amount: BalanceOf<T>, is_add: bool) -> Result<(), ()> {
        let mut committee_stake = Self::committee_stake(&committee);
        let stake_params = Self::committee_stake_params().ok_or(())?;
        let mut all_committee = Self::committee();
        let mut all_committee_changed = false;

        // 还未被使用的质押数量
        let free_stake = committee_stake.staked_amount.checked_sub(&committee_stake.used_stake).ok_or(())?;

        // 计算下一阶段需要的质押数量
        if is_add {
            committee_stake.used_stake = committee_stake.used_stake.checked_add(&amount).ok_or(())?;

            // 检查是否不够最低质押
            if committee_stake.used_stake >= stake_params.min_free_stake_percent * committee_stake.staked_amount {
                // 判断是不是需要补充质押, 如果够了，则可能需要改变委员会状态
                all_committee_changed = true;
                CommitteeList::rm_one(&mut all_committee.normal, &committee);
                CommitteeList::add_one(&mut all_committee.fulfilling_list, committee.clone());
            }

            // if new_free_stake <= stake_params.min_free_stake_percent {}
        } else {
            committee_stake.used_stake = committee_stake.used_stake.checked_sub(&amount).ok_or(())?;
            // 判断是不是够，如果够了，则可能需要改变委员会状态
            if committee_stake.used_stake < stake_params.min_free_stake_percent * committee_stake.staked_amount {
                if let Ok(index) = all_committee.fulfilling_list.binary_search(&committee) {
                    all_committee.fulfilling_list.remove(index);
                    if let Err(index) = all_committee.normal.binary_search(&committee) {
                        all_committee_changed = true;
                        all_committee.normal.insert(index, committee.clone())
                    }
                }
            }
        };

        if all_committee_changed {
            Committee::<T>::put(all_committee);
        }
        CommitteeStake::<T>::insert(&committee, committee_stake);

        Ok(())
    }

    fn stake_per_order() -> Option<BalanceOf<T>> {
        let committee_stake_params = Self::committee_stake_params()?;
        Some(committee_stake_params.stake_per_order)
        // Self::committee_stake_per_order()
    }

    fn add_reward(committee: T::AccountId, reward: BalanceOf<T>) {
        let mut committee_stake = Self::committee_stake(&committee);
        committee_stake.can_claim_reward += reward;
        CommitteeStake::<T>::insert(&committee, committee_stake);
    }

    fn add_slash(who: T::AccountId, amount: BalanceOf<T>, reward_to: Vec<T::AccountId>) {
        let slash_id = Self::get_new_slash_id();
        let now = <frame_system::Module<T>>::block_number();
        PendingSlash::<T>::insert(
            slash_id,
            PendingSlashInfo {
                slash_who: who,
                slash_time: now,
                unlock_amount: amount,
                slash_amount: amount,
                slash_exec_time: now + 5760u32.saturated_into::<T::BlockNumber>(),
                reward_to,
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
