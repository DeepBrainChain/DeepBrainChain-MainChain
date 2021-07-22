#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
    ensure,
    pallet_prelude::*,
    traits::{Currency, LockIdentifier, LockableCurrency, OnUnbalanced, WithdrawReasons},
    IterableStorageMap,
};
use frame_system::pallet_prelude::*;
use online_profile_machine::{DbcPrice, ManageCommittee};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
    traits::{CheckedAdd, CheckedSub, SaturatedConversion},
    RuntimeDebug,
};
use sp_std::{collections::btree_set::BTreeSet, prelude::*, str, vec::Vec};

pub type SlashId = u64;
type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as pallet::Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

pub const PALLET_LOCK_ID: LockIdentifier = *b"committe";

// 即将被执行的罚款
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct PendingSlashInfo<AccountId, BlockNumber, Balance> {
    pub slash_who: AccountId,
    pub slash_time: BlockNumber,      // 惩罚被创建的时间
    pub unlock_amount: Balance,       // 执行惩罚前解绑的金额
    pub slash_amount: Balance,        // 执行惩罚的金额
    pub slash_exec_time: BlockNumber, // 惩罚被执行的时间
    pub reward_to: Vec<AccountId>,    // 奖励发放对象。如果为空，则惩罚到国库
}

// 处于不同状态的委员会的列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct CommitteeList<AccountId: Ord> {
    pub normal: Vec<AccountId>,     // 质押并通过社区选举的委员会，正常状态
    pub chill_list: Vec<AccountId>, // 委员会，但不想被派单
    pub waiting_box_pubkey: Vec<AccountId>, // 等待提交box pubkey的委员会
}

impl<AccountId: Ord> CommitteeList<AccountId> {
    fn exist_in_committee(&self, who: &AccountId) -> bool {
        if let Ok(_) = self.normal.binary_search(who) {
            return true;
        }
        if let Ok(_) = self.chill_list.binary_search(who) {
            return true;
        }
        if let Ok(_) = self.waiting_box_pubkey.binary_search(who) {
            return true;
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
        type DbcPrice: DbcPrice<BalanceOf = BalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_block_number: T::BlockNumber) {
            if let Some(stake_dbc_value) = Self::committee_stake_usd_per_order() {
                if let Some(stake_dbc_amount) =
                    T::DbcPrice::get_dbc_amount_by_value(stake_dbc_value)
                {
                    CommitteeStakeDBCPerOrder::<T>::put(stake_dbc_amount);
                }
            };

            Self::check_and_exec_slash();
        }
    }

    // 每次订单质押默认100RMB等价DBC
    #[pallet::storage]
    #[pallet::getter(fn committee_stake_usd_per_order)]
    pub(super) type CommitteeStakeUSDPerOrder<T: Config> = StorageValue<_, u64>;

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
    #[pallet::getter(fn box_pubkey)]
    pub(super) type BoxPubkey<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, [u8; 32], ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn committee)]
    pub(super) type Committee<T: Config> = StorageValue<_, CommitteeList<T::AccountId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn user_total_stake)]
    pub(super) type UserTotalStake<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>>;

    // 每次订单默认质押等价的DBC数量，每个块更新一次
    #[pallet::storage]
    #[pallet::getter(fn committee_stake_dbc_per_order)]
    pub(super) type CommitteeStakeDBCPerOrder<T: Config> = StorageValue<_, BalanceOf<T>>;

    #[pallet::storage]
    #[pallet::getter(fn committee_reward)]
    pub(super) type CommitteeReward<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置committee每次操作需要质押数量, 单位为usd * 10^6
        #[pallet::weight(0)]
        pub fn set_staked_usd_per_order(
            origin: OriginFor<T>,
            value: u64,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            CommitteeStakeUSDPerOrder::<T>::put(value);
            Ok(().into())
        }

        // 该操作由社区决定
        // 添加到委员会，直接添加到fulfill列表中。每次finalize将会读取委员会币数量，币足则放到committee中
        #[pallet::weight(0)]
        pub fn add_committee(
            origin: OriginFor<T>,
            member: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut committee = Self::committee();
            // 确保用户还未加入到本模块
            ensure!(!committee.exist_in_committee(&member), Error::<T>::AccountAlreadyExist);
            // 将用户添加到waiting_box_pubkey列表中
            CommitteeList::add_one(&mut committee.waiting_box_pubkey, member.clone());

            Committee::<T>::put(committee);
            Self::deposit_event(Event::CommitteeAdded(member));
            Ok(().into())
        }

        // 委员会需要手动添加自己的加密公钥信息
        #[pallet::weight(0)]
        pub fn committee_set_box_pubkey(
            origin: OriginFor<T>,
            box_pubkey: [u8; 32],
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let mut committee_list = Self::committee();

            // 确保是委员会才能执行该操作
            ensure!(committee_list.exist_in_committee(&committee), Error::<T>::NotCommittee);

            BoxPubkey::<T>::insert(&committee, box_pubkey);

            if committee_list.waiting_box_pubkey.binary_search(&committee).is_ok() {
                CommitteeList::rm_one(&mut committee_list.waiting_box_pubkey, &committee);
                CommitteeList::add_one(&mut committee_list.normal, committee);
                Committee::<T>::put(committee_list);
            }

            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn claim_reward(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let can_claim_reward =
                Self::committee_reward(&committee).ok_or(Error::<T>::NoRewardCanClaim)?;

            <T as pallet::Config>::Currency::deposit_into_existing(&committee, can_claim_reward)
                .map_err(|_| Error::<T>::ClaimRewardFailed)?;

            CommitteeReward::<T>::insert(committee, 0u32.saturated_into::<BalanceOf<T>>());

            Ok(().into())
        }

        // 委员会停止接单
        #[pallet::weight(10000)]
        pub fn chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;

            let mut committee_list = Self::committee();
            ensure!(committee_list.exist_in_committee(&committee), Error::<T>::NotCommittee);

            // waiting_box_pubkey不能执行该操作
            if committee_list.waiting_box_pubkey.binary_search(&committee).is_ok() {
                return Err(Error::<T>::PubkeyNotSet.into());
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
            committee_list
                .chill_list
                .binary_search(&who)
                .map_err(|_| Error::<T>::NotInChillList)?;

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

            let mut committee_list = Self::committee();
            ensure!(committee_list.exist_in_committee(&committee), Error::<T>::NotCommittee);

            if let Some(committee_stake) = Self::user_total_stake(&committee) {
                ensure!(
                    committee_stake == 0u64.saturated_into::<BalanceOf<T>>(),
                    Error::<T>::JobNotDone
                );
            }

            CommitteeList::rm_one(&mut committee_list.normal, &committee);
            CommitteeList::rm_one(&mut committee_list.chill_list, &committee);
            CommitteeList::rm_one(&mut committee_list.waiting_box_pubkey, &committee);

            Committee::<T>::put(committee_list);

            Self::deposit_event(Event::ExitFromCandidacy(committee));

            return Ok(().into());
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
    }
}

impl<T: Config> Pallet<T> {
    // 检查并执行slash
    fn check_and_exec_slash() {
        let now = <frame_system::Module<T>>::block_number();

        let pending_slash_id = Self::get_slash_id();
        for a_slash_id in pending_slash_id {
            let a_slash_info = Self::pending_slash(&a_slash_id);
            if now >= a_slash_info.slash_exec_time {
                let _ =
                    Self::change_stake(&a_slash_info.slash_who, a_slash_info.unlock_amount, false);

                // 如果reward_to为0，则将币转到国库
                if a_slash_info.reward_to.len() == 0 {
                    if <T as pallet::Config>::Currency::can_slash(
                        &a_slash_info.slash_who,
                        a_slash_info.slash_amount,
                    ) {
                        let (imbalance, missing) = <T as pallet::Config>::Currency::slash(
                            &a_slash_info.slash_who,
                            a_slash_info.slash_amount,
                        );
                        Self::deposit_event(Event::Slash(
                            a_slash_info.slash_who.clone(),
                            a_slash_info.slash_amount,
                        ));
                        Self::deposit_event(Event::MissedSlash(
                            a_slash_info.slash_who,
                            missing.clone(),
                        ));
                        <T as pallet::Config>::Slash::on_unbalanced(imbalance);
                    }
                } else {
                    // TODO: reward_to将获得slash的奖励
                }
            }
        }
    }

    // 获得所有被惩罚的订单列表
    fn get_slash_id() -> BTreeSet<SlashId> {
        <PendingSlash<T> as IterableStorageMap<SlashId, _>>::iter()
            .map(|(slash_id, _)| slash_id)
            .collect::<BTreeSet<_>>()
    }

    fn get_new_slash_id() -> SlashId {
        let slash_id = Self::next_slash_id();
        NextSlashId::<T>::put(slash_id + 1);
        return slash_id;
    }
}

impl<T: Config> ManageCommittee for Pallet<T> {
    type AccountId = T::AccountId;
    type BalanceOf = BalanceOf<T>;

    // 检查是否为状态正常的委员会
    fn is_valid_committee(who: &T::AccountId) -> bool {
        let committee_list = Self::committee();
        if let Ok(_) = committee_list.normal.binary_search(&who) {
            return true;
        }
        return false;
    }

    // 检查委员会是否有足够的质押,返回有可以抢单的机器列表
    // 在每个区块以及每次分配一个机器之后，都需要检查
    fn available_committee() -> Result<Vec<T::AccountId>, ()> {
        let committee_list = Self::committee();
        let stake_per_gpu = Self::committee_stake_dbc_per_order().ok_or(())?;

        let normal_committee = committee_list.normal.clone();

        let mut out = Vec::new();

        // 如果free_balance足够，则复制到out列表中
        for a_committee in normal_committee {
            // 当委员会质押不够时，将委员会移动到fulfill_list中
            if <T as Config>::Currency::free_balance(&a_committee) > stake_per_gpu {
                out.push(a_committee.clone());
            }
        }

        if out.len() > 0 {
            return Ok(out);
        }
        return Err(());
    }

    fn change_stake(
        controller: &T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
    ) -> Result<(), ()> {
        let total_stake = Self::user_total_stake(&controller).unwrap_or(0u32.into());

        if is_add && <T as Config>::Currency::free_balance(&controller) <= amount {
            return Err(());
        }

        let new_stake = if is_add {
            total_stake.checked_add(&amount).ok_or(())?
        } else {
            total_stake.checked_sub(&amount).ok_or(())?
        };

        <T as Config>::Currency::set_lock(
            PALLET_LOCK_ID,
            controller,
            new_stake,
            WithdrawReasons::all(),
        );
        UserTotalStake::<T>::insert(controller, new_stake);

        Ok(())
    }

    fn stake_per_order() -> Option<BalanceOf<T>> {
        Self::committee_stake_dbc_per_order()
    }

    fn add_reward(committee: T::AccountId, reward: BalanceOf<T>) {
        let raw_reward =
            Self::committee_reward(&committee).unwrap_or(0u32.saturated_into::<BalanceOf<T>>());
        CommitteeReward::<T>::insert(&committee, raw_reward + reward);
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
