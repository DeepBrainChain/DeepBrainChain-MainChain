#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
    dispatch::DispatchResult,
    ensure,
    pallet_prelude::*,
    traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons},
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use online_profile::types::*;
use online_profile_machine::LCOps;
use sp_io::hashing::blake2_128;
use sp_runtime::{traits::SaturatedConversion, RuntimeDebug};
use sp_std::{collections::vec_deque::VecDeque, prelude::*, str};

mod committee;

type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct PendingVerify<BlockNumber> {
    pub machine_id: MachineId,
    pub add_height: BlockNumber,
}

pub const PALLET_LOCK_ID: LockIdentifier = *b"leasecom";

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeMachineList {
    pub booked_machine: Vec<MachineId>,
    pub hashed_machine: Vec<MachineId>, // 存储已经提交了Hash信息的机器
    pub confirmed_machine: Vec<MachineId>, // 存储已经提交了原始确认数据的机器
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineCommitteeList<AccountId> {
    pub booked_committee: Vec<AccountId>,
    pub hashed_committee: Vec<AccountId>,
    pub confirmed_committee: Vec<AccountId>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeMachineOps<BlockNumber> {
    pub booked_time: BlockNumber,
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    pub confirm_raw: Vec<u8>,
    pub confirm_time: BlockNumber,
    pub machine_status: MachineStatus,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MachineStatus {
    Booked,
    Hashed,
    Confirmed,
}

impl Default for MachineStatus {
    fn default() -> Self {
        MachineStatus::Booked
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config + random_num::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type CommitteeMachine: LCOps<AccountId = Self::AccountId, MachineId = MachineId>;

        type OnlineProfile: LCOps<MachineId = MachineId>;

        // #[pallet::constant]
        // type CommitteeDuration: Get<EraIndex>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(block_number: T::BlockNumber) {
            // if Candidacy::<T>::get().len() > 0 && Committee::<T>::get().len() == 0 {
            //     Self::update_committee();
            //     return;
            // }

            // let committee_duration = T::CommitteeDuration::get();
            // let block_per_era = <random_num::Module<T>>::block_per_era();

            // if block_number.saturated_into::<u64>() / (block_per_era * committee_duration) as u64
            //     == 0
            // {
            //     Self::update_committee()
            // }
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

    /// Minmum stake amount to become candidacy
    #[pallet::storage]
    #[pallet::getter(fn committee_min_stake)]
    pub(super) type CommitteeMinStake<T> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    // 候选委员会不设置个数限制,满足质押，并且通过议案选举即可
    // #[pallet::type_value]
    // pub fn CandidacyLimitDefault<T: Config>() -> u32 {
    //     20
    // }

    // #[pallet::storage]
    // #[pallet::getter(fn candidacy_limit)]
    // pub(super) type CandidacyLimit<T: Config> =
    //     StorageValue<_, u32, ValueQuery, CandidacyLimitDefault<T>>;

    // #[pallet::type_value]
    // pub fn CommitteeLimitDefault<T: Config>() -> u32 {
    //     3
    // }

    // // 存储一个机器最多有多少委员会进行抢单验证
    // #[pallet::storage]
    // #[pallet::getter(fn committee_limit)] // pub(super) type CommitteeLimit<T: Config> =
    //     StorageValue<_, u32, ValueQuery, CommitteeLimitDefault<T>>;

    // candidacy, 一定的周期后，从中选出committee来进行机器的认证。
    #[pallet::storage]
    #[pallet::getter(fn candidacy)]
    pub(super) type Candidacy<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    // committee, 进行机器的认证
    // 通过提交议案，通过议案成为委员会
    #[pallet::storage]
    #[pallet::getter(fn committee)]
    pub(super) type Committee<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    // 存储用户订阅的不同确认阶段的机器
    #[pallet::storage]
    #[pallet::getter(fn committee_machine)]
    pub(super) type CommitteeMachine<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, CommitteeMachineList, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn machine_committee)]
    pub(super) type MachineCommittee<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, MachineCommitteeList<T::AccountId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn committee_machine_ops_detail)]
    pub(super) type CommitteeMachineOpsDetail<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        MachineId,
        CommitteeMachineOps<T::BlockNumber>,
        ValueQuery,
    >;

    // // 添加item时使用binray_search
    // #[pallet::storage]
    // #[pallet::getter(fn committee_booking_list)]
    // pub type CommitteeBookList<T: Config> =
    //     StorageMap<_, Blake2_128Concat, T::AccountId, Vec<MachineId>, ValueQuery>;

    // // 存储第一阶段用户的Hash
    // #[pallet::storage]
    // #[pallet::getter(fn committee_hash)]
    // pub type CommitteeHash<T: Config> =
    //     StorageMap<_, Blake2_128Concat, MachineId, Vec<MachineId>, ValueQuery>;

    // 存储第二阶段用户的Hash

    // #[pallet::storage]
    // #[pallet::getter(fn committee_booked_machine)]
    // pub type BookedMachineInfo<T: Config> = StorageMap<
    //     _,
    //     Blake2_128Concat,
    //     MachineId,
    //     BookingItem<<T as frame_system::Config>::BlockNumber>,
    //     ValueQuery,
    // >;

    #[pallet::storage]
    #[pallet::getter(fn chill_list)]
    pub(super) type ChillList<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn black_list)]
    pub(super) type BlackList<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn white_list)]
    pub(super) type WhiteList<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

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
        // 设置committee的最小质押
        /// set min stake to become candidacy
        #[pallet::weight(0)]
        pub fn set_min_stake(
            origin: OriginFor<T>,
            value: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            CommitteeMinStake::<T>::put(value);
            Ok(().into())
        }

        // #[pallet::weight(0)]
        // pub fn set_candidacy_limit(origin: OriginFor<T>, num: u32) -> DispatchResultWithPostInfo {
        //     ensure_root(origin)?;
        //     CandidacyLimit::<T>::put(num);

        //     Ok(().into())
        // }

        // #[pallet::weight(0)]
        // pub fn set_committee_limit(origin: OriginFor<T>, num: u32) -> DispatchResultWithPostInfo {
        //     ensure_root(origin)?;
        //     CommitteeLimit::<T>::put(num);
        //     Ok(().into())
        // }

        /// user can be candidacy by staking
        #[pallet::weight(10000)]
        pub fn stake_for_candidacy(
            origin: OriginFor<T>,
            value: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            // 质押数量应该不小于最小质押要求
            ensure!(
                value >= Self::committee_min_stake(),
                Error::<T>::StakeNotEnough
            );
            // 检查用户余额
            ensure!(
                value < <T as Config>::Currency::free_balance(&who),
                Error::<T>::FreeBalanceNotEnough
            );

            // 该用户不是候选委员会, 且不是委员会成员
            ensure!(!Self::is_candidacy(&who), Error::<T>::AlreadyCandidacy);
            ensure!(!Self::is_committee(&who), Error::<T>::AlreadyCommittee);

            let current_era = <random_num::Module<T>>::current_era();
            let history_depth = Self::history_depth();
            // last_reward_era记录委员会上次领取奖励的时间
            let last_reward_era = current_era.saturating_sub(history_depth);

            let item = StakingLedger {
                stash: who.clone(),
                total: value,
                active: value,
                unlocking: vec![],
                claimed_rewards: (last_reward_era..current_era).collect(),
                released_rewards: 0u32.into(),
                upcoming_rewards: VecDeque::new(),
            };

            Self::update_ledger(&who, &item);
            // 添加到到候选委员会列表，用户进行提案，通过后可以成为委员会成员
            Self::add_to_candidacy(&who)?;

            Self::deposit_event(Event::StakeToBeCandidacy(who, value));
            Ok(().into())
        }

        // 取消作为验证人,并执行unbond
        #[pallet::weight(10000)]
        pub fn chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut chill_list = Self::chill_list();
            let committee = Self::committee();
            let candidacy = Self::candidacy();

            // 确保调用该方法的用户已经在候选委员会列表
            ensure!(candidacy.contains(&who), Error::<T>::NotCandidacy);

            // 如果当前候选人已经在committee列表，则先加入到chill_list中，等到下一次选举时，可以退出
            if committee.contains(&who) {
                // 确保用户不在chill_list中
                ensure!(!chill_list.contains(&who), Error::<T>::AlreadyInChillList);
                chill_list.push(who.clone());
                ChillList::<T>::put(chill_list);
                Self::deposit_event(Event::Chill(who));
                return Ok(().into());
            }

            // 否则将用户从candidacy中移除
            Self::rm_from_candidacy(&who)?;

            let mut ledger = Self::committee_ledger(&who).ok_or(Error::<T>::NotCandidacy)?;
            let era = <random_num::Module<T>>::current_era() + T::BondingDuration::get();
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
            let mut ledger = Self::committee_ledger(&who).ok_or(Error::<T>::NotCandidacy)?;
            let old_total = ledger.total;
            let current_era = <random_num::Module<T>>::current_era();

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

        // 提前预订订单
        #[pallet::weight(10000)]
        pub fn book_one(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            ensure!(Self::is_committee(&who), Error::<T>::NotCommittee);

            let live_machines = <online_profile::Pallet<T>>::live_machines();
            let bonding_machine = live_machines.bonding_machine;

            // 将状态设置为已被订阅状态
            T::OnlineProfile::lc_add_booked_machine(machine_id.clone());

            // TODO: 更新本地存储，记录用户即将要审查的机器

            // 本模块将会缓存委员会提交的结果，并等待所有委员会结果汇总之后，提交给online_profile模块

            // let book_result = T::CommitteeMachine::book_one_machine(&who, machine_id.clone());
            // ensure!(book_result, Error::<T>::BookFailed);

            // Self::add_to_committee_book_list(&who, machine_id.clone());

            // let booking_item = BookingItem {
            //     machine_id: machine_id.clone(),
            //     book_time: <frame_system::Module<T>>::block_number(),
            // };
            // BookedMachineInfo::<T>::insert(machine_id, booking_item);

            // TODO: 如果过了期限，则需要进入到下一阶段
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn book_all(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            // let committee = Self::committee();
            // ensure!(committee.contains(&who), Error::<T>::NotCommittee);

            // let bonding_queue_id = T::CommitteeMachine::bonding_queue_id();
            // for a_machine_id in bonding_queue_id.iter() {
            //     let book_result =
            //         T::CommitteeMachine::book_one_machine(&who, a_machine_id.to_vec());
            //     ensure!(book_result, Error::<T>::BookFailed);

            //     let booking_item = BookingItem {
            //         machine_id: a_machine_id.to_vec(),
            //         book_time: <frame_system::Module<T>>::block_number(),
            //     };
            //     BookedMachineInfo::<T>::insert(a_machine_id.to_vec(), booking_item); // TODO: 可以优化一次存入
            //     Self::add_to_committee_book_list(&who, a_machine_id.to_vec());
            // }

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

        // Root权限，将candidacy中的成员，添加到委员会
        #[pallet::weight(0)]
        pub fn add_committee(
            origin: OriginFor<T>,
            member: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let mut committee = Committee::<T>::get();
            if let Err(index) = committee.binary_search(&member) {
                committee.insert(index, member.clone());
            }

            Committee::<T>::put(committee);
            Self::deposit_event(Event::CommitteeAdded(member));
            Ok(().into())
        }

        // // 重新选择一组委员会
        // #[pallet::weight(0)]
        // pub fn reelection_committee(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
        //     ensure_root(origin)?;
        //     Self::update_committee();
        //     Ok(().into())
        // }

        // 添加确认hash
        #[pallet::weight(10000)]
        fn add_confirm_hash(
            origin: OriginFor<T>,
            machine_id: MachineId,
            hash: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            //检查：
            // 检查：是委员会
            ensure!(Self::is_committee(&who), Error::<T>::NotCommittee);
            // 2. 没有提交过信息
            let mut committee_ops_detail = Self::committee_machine_ops_detail(&who, &machine_id);
            ensure!(
                committee_ops_detail.machine_status == MachineStatus::Booked,
                Error::<T>::AlreadySubmitHash
            );

            // 更改CommitteeMachine, MachineCommittee两个变量
            let mut committee_machine = Self::committee_machine(&who);
            if let Ok(index) = committee_machine.booked_machine.binary_search(&machine_id) {
                committee_machine.booked_machine.remove(index);

                if let Err(index) = committee_machine.hashed_machine.binary_search(&machine_id) {
                    committee_machine.hashed_machine.insert(index, hash);
                }
            } else {
                return Ok(().into());
            }
            CommitteeMachine::<T>::insert(&who, committee_machine);

            let mut machine_committee = Self::machine_committee(&machine_id);
            if let Ok(index) = machine_committee.booked_committee.binary_search(&who) {
                machine_committee.booked_committee.remove(index);

                if let Err(index) = machine_committee.hashed_committee.binary_search(&who) {
                    machine_committee
                        .hashed_committee
                        .insert(index, who.clone());
                }
            } else {
                return Ok(().into());
            }
            MachineCommittee::<T>::insert(&machine_id, machine_committee);

            committee_ops_detail.machine_status = MachineStatus::Hashed;
            committee_ops_detail.hash_time = <frame_system::Module<T>>::block_number();
            // TODO: 添加hash [u8; 16];
            // committee_ops_detail.confirm_hash = "";

            CommitteeMachineOpsDetail::<T>::insert(&who, &machine_id, committee_ops_detail);

            Ok(().into())
        }

        #[pallet::weight(10000)]
        fn add_confirm_raw(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            Ok(().into())
        }

        /// TODO: committee confirm machine grade
        #[pallet::weight(10000)]
        pub fn confirm_machine_grades(
            origin: OriginFor<T>,
            machine_id: MachineId,
            confirm: bool,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            // 用户为委员会
            ensure!(Self::is_committee(&who), Error::<T>::NotCommittee);
            // 需要在committee的booking list中
            ensure!(
                Self::committee_machine_ops_detail(&who, &machine_id).machine_status
                    == MachineStatus::Hashed,
                Error::<T>::NotInBookList
            );

            // let machine_grades = online_profile::OCWMachineGrades::<T>::get(machine_id.clone());
            // TODO: 检查是否已经验证过了
            // TODO: 检查是否已经过了验证时间
            // TODO: 委员会将提交一串Hash函数，在所有委员会都通过之后，再提交原始信息，证明自己的结果
            // for a_machine_info in &machine_meta.committee_confirm {
            //     if a_machine_info.committee == who {
            //         return Err(Error::<T>::CommitteeConfirmedYet.into());
            //     }
            // }

            T::CommitteeMachine::confirm_machine_grade(who, machine_id.clone(), confirm);

            Ok(().into())
        }

        // #[pallet::weight(0)]
        // pub fn set_machine_grade(
        //     origin: OriginFor<T>,
        //     machine_id: MachineId,
        //     grade: u64,
        // ) -> DispatchResultWithPostInfo {
        //     let _ = ensure_root(origin);

        //     if !online_profile::MachineDetail::<T>::contains_key(&machine_id) {
        //         // TODO: 通过trait进行修改
        //         online_profile::MachineDetail::<T>::insert(
        //             &machine_id,
        //             MachineMeta {
        //                 machine_price: 0,
        //                 machine_grade: grade,
        //                 committee_confirm: vec![],
        //             },
        //         );
        //         return Ok(().into());
        //     }

        //     let mut machine_detail = online_profile::MachineDetail::<T>::get(&machine_id);
        //     machine_detail.machine_grade = grade;
        //     online_profile::MachineDetail::<T>::insert(&machine_id, machine_detail);

        //     Ok(().into())
        // }

        // TODO: 增加root操作未被委员会最终确认的机器
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        StakeToBeCandidacy(T::AccountId, BalanceOf<T>),
        CommitteeAdded(T::AccountId),
        CommitteeRemoved(T::AccountId),
        CandidacyAdded(T::AccountId),
        CandidacyRemoved(T::AccountId),
        Chill(T::AccountId),
        Withdrawn(T::AccountId, BalanceOf<T>),
        AddToWhiteList(T::AccountId),
        AddToBlackList(T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        CandidacyLimitReached,
        AlreadyCandidacy,
        NotCandidacy,
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
        NotInBookList,
        BookFailed,
        AlreadySubmitHash,
    }
}

impl<T: Config> Pallet<T> {
    // fn book_one_item(who: &T::AccountId, machine_id: MachineId) -> DispatchResult {
    //     let booking_queue_id = T::CommitteeMachine::booking_queue_id();

    //     // TODO: not work here
    //     // ensure!(
    //     //     booking_queue_id.contains_key(&machine_id),
    //     //     Error::<T>::NotInBookingList
    //     // );

    //     Ok(())
    // }

    fn is_candidacy(who: &T::AccountId) -> bool {
        let candidacy = Self::candidacy();
        if let Ok(_) = candidacy.binary_search(who) {
            return true;
        }
        return false;
    }

    fn is_committee(who: &T::AccountId) -> bool {
        let committee = Committee::<T>::get();
        if let Ok(_) = committee.binary_search(who) {
            return true;
        }
        return false;
    }

    // TODO: 实现该函数
    fn hash_is_identical() -> bool {
        let hash: [u8; 16] = blake2_128(&b"Hello world!"[..]);

        true
    }

    fn add_book_list(who: T::AccountId, machine_id: MachineId) {
        let mut committee_machine = Self::committee_machine(&who);
        let mut machine_committee = Self::machine_committee(&machine_id);
        let mut committee_machine_ops_detail =
            Self::committee_machine_ops_detail(&who, &machine_id);

        if let Err(index) = committee_machine.booked_machine.binary_search(&machine_id) {
            committee_machine
                .booked_machine
                .insert(index, machine_id.to_vec());
        }
        if let Err(index) = machine_committee.booked_committee.binary_search(&who) {
            machine_committee
                .booked_committee
                .insert(index, who.clone());
        }
        committee_machine_ops_detail.booked_time = <frame_system::Module<T>>::block_number();
        committee_machine_ops_detail.machine_status = MachineStatus::Booked;

        CommitteeMachine::<T>::insert(&who, committee_machine);
        MachineCommittee::<T>::insert(&machine_id, machine_committee);
        CommitteeMachineOpsDetail::<T>::insert(who, machine_id, committee_machine_ops_detail);
    }

    fn add_to_candidacy(who: &T::AccountId) -> DispatchResult {
        let mut candidacy = Self::candidacy();

        if let Err(index) = candidacy.binary_search(who) {
            candidacy.insert(index, who.clone());
            Candidacy::<T>::put(candidacy);
        }

        Ok(())
    }

    fn rm_from_candidacy(who: &T::AccountId) -> DispatchResult {
        let mut candidacy = Self::candidacy();

        match candidacy.binary_search(who) {
            Ok(index) => {
                candidacy.remove(index);
                Candidacy::<T>::put(candidacy);
                Ok(())
            }
            Err(_) => Err(Error::<T>::NotCandidacy.into()),
        }
    }

    // // 产生一组随机的审核委员会，并更新
    // // TODO: 排除黑名单用户，增加白名单用户
    // fn update_committee() {
    //     let mut candidacy = Candidacy::<T>::get();
    //     let committee_num = CommitteeLimit::<T>::get();
    //     let mut next_group = Vec::new();

    //     if candidacy.len() == 0 {
    //         return;
    //     }
    //     if candidacy.len() as u32 <= committee_num {
    //         Committee::<T>::put(candidacy);
    //         return;
    //     }

    //     for _ in 0..committee_num {
    //         // TODO: 测试是否可以这样调用吗
    //         let committee_index = <random_num::Module<T>>::random_u32(candidacy.len() as u32 - 1);
    //         next_group.push(candidacy[committee_index as usize].clone());
    //         candidacy.remove(committee_index as usize);
    //     }

    //     Committee::<T>::put(next_group);
    // }

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
