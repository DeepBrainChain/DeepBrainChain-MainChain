#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
    dispatch::DispatchResult,
    ensure,
    traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons},
};
use frame_system::{self as system, ensure_root, ensure_signed};
use online_profile::types::*;
use online_profile_machine::CommitteeMachine;
use sp_runtime::{traits::SaturatedConversion, RuntimeDebug};
use sp_std::{collections::vec_deque::VecDeque, prelude::*, str};

type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as system::Config>::AccountId>>::Balance;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct PendingVerify<BlockNumber> {
    pub machine_id: MachineId,
    pub add_height: BlockNumber,
}

pub const PALLET_LOCK_ID: LockIdentifier = *b"leasecom";

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type CommitteeMachine: CommitteeMachine;

        #[pallet::constant]
        type CommitteeDuration: Get<EraIndex>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(block_number: T::BlockNumber) {
            if AlternateCommittee::<T>::get().len() > 0 && Committee::<T>::get().len() == 0 {
                Self::update_committee();
                return;
            }

            // let current_era = online_profile::Module::<T>::current_era();
            let committee_duration = T::CommitteeDuration::get();
            let block_per_era = online_profile::Module::<T>::block_per_era();

            if block_number.saturated_into::<u64>() / (block_per_era * committee_duration) as u64
                == 0
            {
                Self::update_committee()
            }
        }
    }

    #[pallet::type_value]
    pub fn HistoryDepthDefault<T: Config>() -> u32 {
        150
    }

    #[pallet::storage]
    #[pallet::getter(fn history_depth)]
    pub(super) type HistoryDepth<T: Config> =
        StorageValue<_, u32, ValueQuery, HistoryDepthDefault<T>>;

    // Minmum stake amount to become alternateCommittee
    #[pallet::storage]
    #[pallet::getter(fn committee_min_stake)]
    pub(super) type CommitteeMinStake<T> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::type_value]
    pub fn AlternateCommitteeLimitDefault<T: Config>() -> u32 {
        20
    }

    #[pallet::storage]
    #[pallet::getter(fn alternate_committee_limit)]
    pub(super) type AlternateCommitteeLimit<T: Config> =
        StorageValue<_, u32, ValueQuery, AlternateCommitteeLimitDefault<T>>;

    #[pallet::type_value]
    pub fn CommitteeLimitDefault<T: Config>() -> u32 {
        6
    }

    #[pallet::storage]
    pub(super) type CommitteeLimit<T: Config> =
        StorageValue<_, u32, ValueQuery, CommitteeLimitDefault<T>>;

    // Alternate Committee, 一定的周期后，从中选出committee来进行机器的认证。
    #[pallet::storage]
    #[pallet::getter(fn alternate_committee)]
    pub(super) type AlternateCommittee<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    // committee, 进行机器的认证
    #[pallet::storage]
    #[pallet::getter(fn committee)]
    pub(super) type Committee<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn chill_list)]
    pub(super) type ChillList<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn black_list)]
    pub(super) type BlackList<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn white_list)]
    pub(super) type WhiteList<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    /// epnding verify machine
    #[pallet::storage]
    #[pallet::getter(fn pending_verify_machine)]
    pub(super) type PendingVerifyMachine<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, VecDeque<PendingVerify<T::BlockNumber>>>;

    #[pallet::storage]
    #[pallet::getter(fn committee_ledger)]
    pub(super) type CommitteeLedger<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Option<StakingLedger<T::AccountId, BalanceOf<T>>>,
        ValueQuery,
    >;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // TODO: use in pallet 3.0 type
        // const BondingDuration: EraIndex = <T as Config>::BondingDuration::get();

        // 设置committee的最小质押
        /// set min stake to become alternate committee
        #[pallet::weight(0)]
        pub fn set_min_stake(
            origin: OriginFor<T>,
            value: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            CommitteeMinStake::<T>::put(value);
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn set_alternate_committee_limit(
            origin: OriginFor<T>,
            num: u32,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            AlternateCommitteeLimit::<T>::put(num);
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn set_committee_limit(origin: OriginFor<T>, num: u32) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            CommitteeLimit::<T>::put(num);
            Ok(().into())
        }

        /// user can be alternate_committee by staking
        #[pallet::weight(10000)]
        pub fn stake_for_alternate_committee(
            origin: OriginFor<T>,
            value: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            // 质押数量应该不小于最小质押要求
            ensure!(
                value >= Self::committee_min_stake(),
                Error::<T>::StakeNotEnough
            );
            // 检查余额
            ensure!(
                value < <T as Config>::Currency::free_balance(&who),
                Error::<T>::FreeBalanceNotEnough
            );

            // 不是候选委员会
            let alternate_committee = Self::alternate_committee();
            ensure!(
                !alternate_committee.contains(&who),
                Error::<T>::AlreadyAlternateCommittee
            );
            // 确保候选委员会还未满额
            ensure!(
                alternate_committee.len() < Self::alternate_committee_limit() as usize,
                Error::<T>::AlternateCommitteeLimitReached
            );

            // 不是委员会成员
            let committee = Self::committee();
            ensure!(!committee.contains(&who), Error::<T>::AlreadyCommittee);

            let current_era = online_profile::Module::<T>::current_era();
            let history_depth = Self::history_depth();
            let last_reward_era = current_era.saturating_sub(history_depth);

            let item = StakingLedger {
                stash: who.clone(),
                total: value,
                active: value,
                unlocking: vec![],
                claimed_rewards: (last_reward_era..current_era).collect(),
            };

            Self::update_ledger(&who, &item);
            // 添加到到候选委员会列表，在下次选举时生效
            Self::add_to_alternate_committee(&who)?;

            Self::deposit_event(Event::StakeToBeAlternateCommittee(who, value));
            Ok(().into())
        }

        // 取消作为验证人,并执行unbond
        #[pallet::weight(10000)]
        pub fn chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut chill_list = Self::chill_list();
            let committee = Self::committee();
            let alternate_committee = Self::alternate_committee();

            // 确保调用该方法的用户已经在候选委员会列表
            ensure!(
                alternate_committee.contains(&who),
                Error::<T>::NotAlternateCommittee
            );

            // 如果当前候选人已经在committee列表，则先加入到chill_list中，等到下一次选举时，可以退出
            if committee.contains(&who) {
                // 确保用户不在chill_list中
                ensure!(!chill_list.contains(&who), Error::<T>::AlreadyInChillList);
                chill_list.push(who.clone());
                ChillList::<T>::put(chill_list);
                Self::deposit_event(Event::Chill(who));
                return Ok(().into());
            }

            // 否则将用户从alternate_committee中移除
            Self::rm_from_alternate_committee(&who)?;

            let mut ledger =
                Self::committee_ledger(&who).ok_or(Error::<T>::NotAlternateCommittee)?;
            let era = online_profile::Module::<T>::current_era() + T::BondingDuration::get();
            ledger.unlocking.push(UnlockChunk {
                value: ledger.active,
                era,
            });
            Self::update_ledger(&who, &ledger);

            Ok(().into())
        }

        // 用户chill之后，等到一定时间可以进行withdraw
        #[pallet::weight(10000)]
        fn withdraw_unbonded(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let mut ledger =
                Self::committee_ledger(&who).ok_or(Error::<T>::NotAlternateCommittee)?;
            let old_total = ledger.total;
            let current_era = online_profile::Module::<T>::current_era();

            ledger = ledger.consolidate_unlock(current_era);

            if ledger.unlocking.is_empty()
                && ledger.active <= <T as Config>::Currency::minimum_balance()
            {
                // TODO: 清除ledger相关存储
                <T as Config>::Currency::remove_lock(PALLET_LOCK_ID, &who);
            } else {
                Self::update_ledger(&who, &ledger);
            };

            if ledger.total < old_total {
                let value = old_total - ledger.total;
                Self::deposit_event(Event::Withdrawn(who, value));
            }

            Ok(().into())
        }

        // TODO: 查看需要预订的

        // 提前预订订单
        #[pallet::weight(10000)]
        pub fn booking(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let committee = Self::committee();

            let booking_list = T::CommitteeMachine::booking_queue_id();

            if !committee.contains(&who) {
                return Err(Error::<T>::NotCommittee.into());
            }

            // 抢了单可以放到双端队列中, 先进先出，用户只要点击预订下一个就行了
            Ok(().into())
        }

        /// TODO: committee confirm machine grade
        #[pallet::weight(0)]
        pub fn confirm_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            confirm: bool,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            ensure!(Self::committee().contains(&who), Error::<T>::NotCommittee);
            if !online_profile::MachineDetail::<T>::contains_key(&machine_id) {
                return Err(Error::<T>::MachineGradePriceNotSet.into());
            };
            let mut machine_meta = online_profile::MachineDetail::<T>::get(&machine_id);
            for a_machine_info in &machine_meta.committee_confirm {
                if a_machine_info.committee == who {
                    return Err(Error::<T>::CommitteeConfirmedYet.into());
                }
            }
            let confirm_data = CommitteeConfirm {
                committee: who.clone(),
                confirm: confirm,
            };

            machine_meta.committee_confirm.push(confirm_data);

            // TODO: 增加trait来修改变量！

            // TODO: 使用trait来修改，而不能直接修改
            online_profile::MachineDetail::<T>::insert(&machine_id, machine_meta);
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn add_white_list(
            origin: OriginFor<T>,
            member: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut members = WhiteList::<T>::get();
            // 该用户不在黑名单中
            let black_list = BlackList::<T>::get();
            if let Ok(_) = black_list.binary_search(&member) {
                return Err(Error::<T>::UserInBlackList.into());
            }

            match members.binary_search(&member) {
                Ok(_) => Err(Error::<T>::AlreadyInWhiteList.into()),
                Err(index) => {
                    members.insert(index, member.clone());
                    WhiteList::<T>::put(members);
                    Self::deposit_event(Event::AddToWhiteList(member));
                    Ok(().into())
                }
            }
        }

        #[pallet::weight(0)]
        pub fn rm_white_list(
            origin: OriginFor<T>,
            member: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut members = WhiteList::<T>::get();
            match members.binary_search(&member) {
                Ok(index) => {
                    members.remove(index);
                    WhiteList::<T>::put(members);
                    Ok(().into())
                }
                Err(_) => Err(Error::<T>::NotInWhiteList.into()),
            }
        }

        #[pallet::weight(0)]
        pub fn add_black_list(
            origin: OriginFor<T>,
            member: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut members = BlackList::<T>::get();
            let white_list = WhiteList::<T>::get();
            if let Ok(_) = white_list.binary_search(&member) {
                return Err(Error::<T>::UserInWhiteList.into());
            }

            match members.binary_search(&member) {
                Ok(_) => Err(Error::<T>::AlreadyInBlackList.into()),
                Err(index) => {
                    members.insert(index, member.clone());
                    BlackList::<T>::put(members);
                    Self::deposit_event(Event::AddToBlackList(member));
                    Ok(().into())
                }
            }
        }

        #[pallet::weight(0)]
        pub fn rm_black_list(
            origin: OriginFor<T>,
            member: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut members = BlackList::<T>::get();
            match members.binary_search(&member) {
                Ok(index) => {
                    members.remove(index);
                    BlackList::<T>::put(members);
                    Ok(().into())
                }
                Err(_) => Err(Error::<T>::NotInBlackList.into()),
            }
        }

        // 重新选择一组委员会
        #[pallet::weight(0)]
        pub fn reelection_committee(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            Self::update_committee();
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn set_machine_grade(
            origin: OriginFor<T>,
            machine_id: MachineId,
            grade: u64,
        ) -> DispatchResultWithPostInfo {
            let _ = ensure_root(origin);

            if !online_profile::MachineDetail::<T>::contains_key(&machine_id) {
                // TODO: 通过trait进行修改
                online_profile::MachineDetail::<T>::insert(
                    &machine_id,
                    MachineMeta {
                        machine_price: 0,
                        machine_grade: grade,
                        committee_confirm: vec![],
                    },
                );
                return Ok(().into());
            }

            let mut machine_detail = online_profile::MachineDetail::<T>::get(&machine_id);
            machine_detail.machine_grade = grade;
            online_profile::MachineDetail::<T>::insert(&machine_id, machine_detail);

            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        StakeToBeAlternateCommittee(T::AccountId, BalanceOf<T>),
        CommitteeAdded(T::AccountId),
        CommitteeRemoved(T::AccountId),
        AlternateCommitteeAdded(T::AccountId),
        AlternateCommitteeRemoved(T::AccountId),
        Chill(T::AccountId),
        Withdrawn(T::AccountId, BalanceOf<T>),
        AddToWhiteList(T::AccountId),
        AddToBlackList(T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        AlternateCommitteeLimitReached,
        AlreadyAlternateCommittee,
        NotAlternateCommittee,
        CommitteeLimitReached,
        AlreadyCommittee,
        NotCommittee,
        MachineGradePriceNotSet,
        CommitteeConfirmedYet,
        StakeNotEnough,
        FreeBalanceNotEnough,
        AlreadyInChillList,
        UserInBlackList,
        AlreadyInWhiteList,
        NotInWhiteList,
        UserInWhiteList,
        AlreadyInBlackList,
        NotInBlackList,
    }
}

impl<T: Config> Pallet<T> {
    fn add_to_alternate_committee(who: &T::AccountId) -> DispatchResult {
        let mut alternate_committee = Self::alternate_committee();

        match alternate_committee.binary_search(who) {
            Err(i) => {
                alternate_committee.insert(i, who.clone());
                AlternateCommittee::<T>::put(alternate_committee);
                Ok(())
            }
            Ok(_) => Ok(()),
        }
    }

    fn rm_from_alternate_committee(who: &T::AccountId) -> DispatchResult {
        let mut alternate_committee = Self::alternate_committee();

        match alternate_committee.binary_search(who) {
            Ok(index) => {
                alternate_committee.remove(index);
                AlternateCommittee::<T>::put(alternate_committee);
                Ok(())
            }
            Err(_) => Err(Error::<T>::NotAlternateCommittee.into()),
        }
    }

    // 质押一定数量的DBC才能成为候选人
    fn _alternate_committee_stake(_who: T::AccountId, _balance: BalanceOf<T>) {}

    // 产生一组随机的审核委员会，并更新
    // TODO: 排除黑名单用户，增加白名单用户
    fn update_committee() {
        let mut alternate_committee = AlternateCommittee::<T>::get();
        let committee_num = CommitteeLimit::<T>::get();
        let mut next_group = Vec::new();

        if alternate_committee.len() == 0 {
            return;
        }
        if alternate_committee.len() as u32 <= committee_num {
            Committee::<T>::put(alternate_committee);
            return;
        }

        for _ in 0..committee_num {
            let committee_index =
                online_profile::Module::<T>::random_num(alternate_committee.len() as u32 - 1);
            next_group.push(alternate_committee[committee_index as usize].clone());
            alternate_committee.remove(committee_index as usize);
        }

        Committee::<T>::put(next_group);
    }

    fn update_ledger(
        controller: &T::AccountId,
        ledger: &StakingLedger<T::AccountId, BalanceOf<T>>,
    ) {
        <T as Config>::Currency::set_lock(
            PALLET_LOCK_ID,
            &ledger.stash,
            ledger.total,
            WithdrawReasons::all(),
        );
        <CommitteeLedger<T>>::insert(controller, Some(ledger));
    }
}
