// 委员会不设置个数限制，满足质押，并且通过议案选举即可。
// 每次机器加入，系统随机选择3个委员会，对应到0~36h。每次验证区间为4个小时,共有9个验证区间。
// 每个委员随机分得3个验证区间，进行验证。
// 下一轮选择，与上一轮委员会是否被选择的状态无关。
// 委员会确认机器，会提供三个字段组成的 Hash1 = Hash(机器原始信息, 委员会随机字符串, bool(机器正常与否))

// Hash(GPU型号, GPU数量, CUDA core数量, GPU显存, 算力值, 硬盘, 上行带宽, 下行带宽, CPU型号, CPU内核数)

// 最后12个小时，统计委员会结果，多数结果为最终结果。第二次提交信息为： 机器原始信息，委员会随机字符串，bool.
// 验证：1. Hash(机器原始信息) == OCW获取到的机器Hash
//      2. Hash(机器原始信息，委员会随机字符串, bool) == Hash1
// 如果没有人提交信息，则进行新一轮随机派发。

// 钱包地址：xxxx
// 钱包签名信息：xxxx
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, HasCompact};
use frame_support::{
    ensure,
    pallet_prelude::*,
    traits::{Currency, Get, LockIdentifier, LockableCurrency, WithdrawReasons},
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
// use online_profile::types::*;
use online_profile::MachineInfoByCommittee;
use online_profile_machine::LCOps;
use sp_runtime::{traits::SaturatedConversion, RuntimeDebug};
use sp_std::{prelude::*, str, vec::Vec};

pub type MachineId = Vec<u8>;
pub type EraIndex = u32;
type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct PendingVerify<BlockNumber> {
    pub machine_id: MachineId,
    pub add_height: BlockNumber,
}

pub const PALLET_LOCK_ID: LockIdentifier = *b"leasecom";
pub const DISTRIBUTION: u32 = 9; // 分成9个区间进行验证
                                 // pub const DURATIONPERCOMMITTEE: u32 = 480; // 每个用户有480个块的时间验证机器: 480 * 30 / 3600 = 4 hours

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// 记录处于不同状态的委员会的列表，方便派单
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct LCCommitteeList<AccountId: Ord> {
    pub committee: Vec<AccountId>,    // 质押并通过社区选举的委员会
    pub chill_list: Vec<AccountId>,   // 委员会，但不想被派单
    pub fulfill_list: Vec<AccountId>, // 委员会, 但需要补交质押
    pub black_list: Vec<AccountId>,   // 委员会，黑名单中
}

impl<AccountId: Ord> LCCommitteeList<AccountId> {
    fn staker_exist(&self, who: &AccountId) -> bool {
        if let Ok(_) = self.committee.binary_search(who) {
            return true;
        }
        if let Ok(_) = self.chill_list.binary_search(who) {
            return true;
        }
        if let Ok(_) = self.fulfill_list.binary_search(who) {
            return true;
        }
        if let Ok(_) = self.black_list.binary_search(who) {
            return true;
        }
        false
    }

    fn add_staker(a_field: &mut Vec<AccountId>, new_staker: AccountId) {
        if let Err(index) = a_field.binary_search(&new_staker) {
            a_field.insert(index, new_staker);
        }
    }

    fn rm_staker(a_field: &mut Vec<AccountId>, drop_staker: &AccountId) {
        if let Ok(index) = a_field.binary_search(drop_staker) {
            a_field.remove(index);
        }
    }
}

// 记录用户的质押及罚款
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, Default)]
pub struct StakingLedger<Balance: HasCompact> {
    #[codec(compact)]
    pub total: Balance,
    #[codec(compact)]
    pub active: Balance,
}

// 从用户地址查询绑定的机器列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct LCCommitteeMachineList {
    pub booked_machine: Vec<MachineId>, // 记录分配给用户的机器ID及开始验证时间
    pub hashed_machine: Vec<MachineId>, // 存储已经提交了Hash信息的机器
    pub confirmed_machine: Vec<MachineId>, // 存储已经提交了原始确认数据的机器
    pub online_machine: Vec<MachineId>, // 存储已经成功上线的机器
}

// 一台机器对应的委员会
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct LCMachineCommitteeList<AccountId, BlockNumber> {
    pub book_time: BlockNumber,
    pub booked_committee: Vec<AccountId>, // 记录分配给机器的委员会及验证开始时间
    pub hashed_committee: Vec<AccountId>,
    pub confirm_start: BlockNumber, // 开始提交raw信息的时间
    pub confirmed_committee: Vec<AccountId>,
    pub onlined_committee: Vec<AccountId>, // 可以获得该机器在线奖励的委员会
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct LCCommitteeOps<BlockNumber> {
    pub booked_time: BlockNumber,
    pub verify_time: Vec<BlockNumber>,
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    pub confirm_raw: Vec<u8>,
    pub confirm_time: BlockNumber, // 委员会提交raw信息的时间
    pub confirm_result: bool,
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

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum CommitteeStatus<BlockNumber> {
    NotCommittee,          // 非委员会，默认状态
    Health,                // 正常的委员会状态
    FillingPledge,         // 需要等待补充押金
    Chilling(BlockNumber), // 正在退出的状态, 记录Chill时的高度，当达到质押限制时，则可以退出
}

impl<BlockNumber> Default for CommitteeStatus<BlockNumber> {
    fn default() -> Self {
        CommitteeStatus::NotCommittee
    }
}

#[rustfmt::skip]
#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config + random_num::Config + dbc_price_ocw::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type LCOperations: LCOps<AccountId = Self::AccountId, MachineId = MachineId>;
        type BondingDuration: Get<EraIndex>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_block_number: T::BlockNumber) {
            // 分派机器
            Self::distribute_machines();
            Self::statistic_result();

            // TODO: 轮训是否已经有委员会的订单到期，如果到期
            // 则对该成员进行惩罚。并从该委员会的列表中移除该任务

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

    // 最小质押默认100RMB等价DBC
    /// Minmum stake amount to become candidacy (usd * 10**6)
    #[pallet::storage]
    #[pallet::getter(fn committee_min_stake)]
    pub(super) type CommitteeMinStake<T: Config> = StorageValue<_, u64, ValueQuery>;

    // 设定委员会审查所需时间，单位：块高
    #[pallet::type_value]
    pub fn ConfirmTimeLimitDefault<T: Config>() -> T::BlockNumber {
        480u32.into()
    }

    // 记录用户需要在book之后，多少个高度内完成确认
    #[pallet::storage]
    #[pallet::getter(fn confirm_time_limit)]
    pub(super) type ConfirmTimeLimit<T: Config> = StorageValue<_, T::BlockNumber, ValueQuery, ConfirmTimeLimitDefault<T>>;

    #[pallet::storage]
    #[pallet::getter(fn committee)]
    pub(super) type Committee<T: Config> = StorageValue<_, LCCommitteeList<T::AccountId>, ValueQuery>;

    // 存储用户订阅的不同确认阶段的机器
    #[pallet::storage]
    #[pallet::getter(fn committee_machine)]
    pub(super) type CommitteeMachine<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, LCCommitteeMachineList, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn machine_committee)]
    pub(super) type MachineCommittee<T: Config> = StorageMap<_, Blake2_128Concat, MachineId, LCMachineCommitteeList<T::AccountId, T::BlockNumber>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn committee_ops)]
    pub(super) type CommitteeOps<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        MachineId,
        LCCommitteeOps<T::BlockNumber>,
        ValueQuery,
    >;

    // #[pallet::storage]
    // #[pallet::getter(fn black_list)]
    // pub(super) type BlackList<T: Config> = StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn committee_ledger)]
    pub(super) type CommitteeLedger<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Option<StakingLedger<BalanceOf<T>>>,
        ValueQuery,
    >;

    #[pallet::call]
    #[rustfmt::skip]
    impl<T: Config> Pallet<T> {
        // 设置committee的最小质押，一般等于两天的奖励
        /// set min stake to become committee, value: usd * 10^6
        #[pallet::weight(0)]
        pub fn set_min_stake(origin: OriginFor<T>, value: u64) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            CommitteeMinStake::<T>::put(value);
            Ok(().into())
        }

        // 该操作由社区决定
        // Root权限，添加到委员会，直接添加到fulfill列表中。当竞选成功后，需要操作以从fulfill_list到committee
        #[pallet::weight(0)]
        pub fn add_committee(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut staker = Self::committee();

            // 确保用户还未加入到本模块
            ensure!(!staker.staker_exist(&member), Error::<T>::AccountAlreadyExist);

            // 将用户添加到fulfill列表中
            LCCommitteeList::add_staker(&mut staker.fulfill_list, member.clone());
            Self::deposit_event(Event::CommitteeAdded(member));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn fill_pledge(origin: OriginFor<T>) -> DispatchResultWithPostInfo{
            let who = ensure_signed(origin)?;

            // 检查是否在fulfill列表中
            let mut staker = Self::committee();
            if let Err(_) = staker.fulfill_list.binary_search(&who) {
                return Err(Error::<T>::NoNeedFulfill.into());
            }

            // 获取需要质押的数量
            let min_stake = Self::get_min_stake_amount();
            if let None = min_stake {
                return Err(Error::<T>::MinStakeNotFound.into());
            }
            let min_stake = min_stake.unwrap();

            let mut ledger = Self::committee_ledger(&who).unwrap_or(StakingLedger {
                ..Default::default()
            });

            // 检查用户余额，更新质押
            let needed = min_stake - ledger.total;
            ensure!(needed < <T as Config>::Currency::free_balance(&who), Error::<T>::FreeBalanceNotEnough);

            ledger.active += min_stake - ledger.total;
            ledger.total = min_stake;
            Self::update_ledger(&who, &ledger);

            // 从fulfill 移出来，并放到正常委员会列表
            LCCommitteeList::rm_staker(&mut staker.fulfill_list, &who);
            LCCommitteeList::add_staker(&mut staker.committee, who.clone());

            Self::deposit_event(Event::CommitteeFulfill(needed));

            Ok(().into())
        }

        // 委员会停止接单
        #[pallet::weight(10000)]
        pub fn chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut staker = Self::committee();
            ensure!(staker.staker_exist(&who), Error::<T>::AccountNotExist);

            // 只有committee状态才允许进行chill
            if let Err(_) = staker.committee.binary_search(&who) {
                return Err(Error::<T>::NotCommittee.into());
            }

            LCCommitteeList::rm_staker(&mut staker.committee, &who);
            LCCommitteeList::add_staker(&mut staker.chill_list, who.clone());

            Committee::<T>::put(staker);
            Self::deposit_event(Event::Chill(who));

            Ok(().into())
        }

        // 委员会可以接单
        #[pallet::weight(10000)]
        pub fn undo_chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut staker = Self::committee();
            if let Err(_) = staker.chill_list.binary_search(&who) {
                return Err(Error::<T>::NotInChillList.into());
            }

            LCCommitteeList::rm_staker(&mut staker.chill_list, &who);
            LCCommitteeList::add_staker(&mut staker.committee, who.clone());
            Committee::<T>::put(staker);

            Self::deposit_event(Event::UndoChill(who));
            Ok(().into())
        }

        // 委员会可以退出, 从chill_list中退出
        #[pallet::weight(10000)]
        pub fn exit_staker(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut staker = Self::committee();
            ensure!(staker.staker_exist(&who), Error::<T>::AccountNotExist);

            // 如果有未完成的工作，则不允许退出
            let committee_machines = Self::committee_machine(&who);
            if committee_machines.booked_machine.len() > 0 ||
                committee_machines.hashed_machine.len() > 0 ||
                committee_machines.confirmed_machine.len() > 0 {
                    return Err(Error::<T>::JobNotDone.into());
            }

            // 如果是candidacy，则可以直接退出, 从staker中删除
            // 如果是fulfill_list则可以直接退出(低于5wDBC的将进入fulfill_list，无法抢单,每次惩罚1w)
            LCCommitteeList::rm_staker(&mut staker.committee, &who);
            LCCommitteeList::rm_staker(&mut staker.fulfill_list, &who);
            LCCommitteeList::rm_staker(&mut staker.chill_list, &who);

            Committee::<T>::put(staker);
            let ledger = Self::committee_ledger(&who);
            if let Some(mut ledger) = ledger {
                ledger.total = 0u32.into();
                Self::update_ledger(&who, &ledger);
            }

            CommitteeLedger::<T>::remove(&who);
            Self::deposit_event(Event::ExitFromCandidacy(who));

            return Ok(().into());
        }

        #[pallet::weight(0)]
        pub fn add_black_list(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let mut staker = Self::committee();
            if staker.staker_exist(&member) {
                LCCommitteeList::rm_staker(&mut staker.committee, &member);
                LCCommitteeList::rm_staker(&mut staker.chill_list, &member);
                LCCommitteeList::rm_staker(&mut staker.fulfill_list, &member);
            }

            LCCommitteeList::add_staker(&mut staker.black_list, member.clone());
            Committee::<T>::put(staker);

            Self::deposit_event(Event::AddToBlackList(member));
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn rm_black_list(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let mut staker = Self::committee();
            // 只从black_list中移出，想要加入委员会，需要社区再次投票
            LCCommitteeList::rm_staker(&mut staker.black_list, &member);

            Self::deposit_event(Event::RmFromBlackList(member));
            Ok(().into())
        }

        // 添加确认hash
        // FIXME: 注意，提交Hash需要检查，不与其他人的/已存在的Hash相同, 否则, 是很严重的作弊行为
        #[pallet::weight(10000)]
        fn add_confirm_hash(origin: OriginFor<T>, machine_id: MachineId, hash: [u8; 16]) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let now = <frame_system::Module<T>>::block_number();

            let mut machine_committee = Self::machine_committee(&machine_id);
            let left_schedule = machine_committee.booked_committee.len();

            // 从机器信息列表中移除所有该委员的任务
            if let Err(_) = machine_committee.booked_committee.binary_search(&who) {
                return Err(Error::<T>::NotInBookList.into());
            }

            // 该机器信息中，记录上委员会已经进行了Hash
            if let Err(index) = machine_committee.hashed_committee.binary_search(&who) {
                machine_committee.hashed_committee.insert(index, who.clone());
            }

            // 从委员的任务中，删除该机器的任务
            let mut committee_machine = Self::committee_machine(&who);
            if let Ok(index) = committee_machine.booked_machine.binary_search(&machine_id) {
                committee_machine.booked_machine.remove(index);
            }

            // 委员会hashedmachine添加上该机器
            if let Err(index) = committee_machine.hashed_machine.binary_search(&machine_id) {
                committee_machine.hashed_machine.insert(index, machine_id.clone());
            }

            // 添加用户对机器的操作记录
            let mut committee_ops = Self::committee_ops(&who, &machine_id);
            committee_ops.machine_status = MachineStatus::Hashed;
            committee_ops.confirm_hash = hash.clone();
            committee_ops.hash_time = now;

            // 更新存储
            MachineCommittee::<T>::insert(&machine_id, machine_committee);
            CommitteeMachine::<T>::insert(&who, committee_machine);
            CommitteeOps::<T>::insert(&who, &machine_id, committee_ops);

            Self::deposit_event(Event::AddConfirmHash(who, hash));

            Ok(().into())
        }

        // fn submit_confirm_raw(origin: OriginFor<T>, machine_id: MachineId, confirm_raw: Vec<u8>) -> DispatchResultWithPostInfo {
        // 委员会提交的原始信息
        #[pallet::weight(10000)]
        fn submit_confirm_raw(
            origin: OriginFor<T>, machine_info_detail: MachineInfoByCommittee, rand_str: Vec<u8>, confirm_raw: Vec<u8>) -> DispatchResultWithPostInfo
        {
            let who = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let machine_id = machine_info_detail.machine_id.clone();

            let mut machine_committee = Self::machine_committee(&machine_id);
            let mut committee_machine = Self::committee_machine(&who);
            let mut machine_ops = Self::committee_ops(&who, &machine_id);

            // 查询是否已经到了提交hash的时间
            ensure!(now >= machine_committee.confirm_start, Error::<T>::TimeNotAllow);
            ensure!(now <= machine_committee.book_time + (3600u32 / 30 * 48).into(), Error::<T>::TimeNotAllow);

            // 该用户已经给机器提交过Hash
            if let Err(_) = machine_committee.hashed_committee.binary_search(&who) {
                return Err(Error::<T>::NotSubmitHash.into());
            }

            // 机器ID存在于用户已经Hash的机器里
            if let Err(_) = committee_machine.hashed_machine.binary_search(&machine_id) {
                return Err(Error::<T>::NotSubmitHash.into());
            }

            // 检查提交的raw与已提交的Hash一致
            let info_hash = machine_info_detail.hash(rand_str);
            ensure!(info_hash == machine_ops.confirm_hash, Error::<T>::NotAllHashSubmited);

            // 用户还未提交过原始信息
            if let Ok(_) = committee_machine.confirmed_machine.binary_search(&machine_id) {
                return Err(Error::<T>::AlreadySubmitRaw.into());
            }

            // 修改存储
            if let Ok(index) = committee_machine.hashed_machine.binary_search(&machine_id) {
                committee_machine.hashed_machine.remove(index);
            }
            if let Err(index) = committee_machine.confirmed_machine.binary_search(&machine_id) {
                committee_machine.confirmed_machine.insert(index, machine_id.clone());
            }

            if let Err(index) = machine_committee.confirmed_committee.binary_search(&who) {
                machine_committee.confirmed_committee.insert(index, who.clone());
            }

            machine_ops.confirm_raw = confirm_raw.clone();
            machine_ops.confirm_time = now;
            machine_ops.machine_status = MachineStatus::Confirmed;

            // 存储用户是否支持该机器
            let confirm_raw = str::from_utf8(&confirm_raw).unwrap();
            machine_ops.confirm_result = if confirm_raw.ends_with("true") {
                true
            } else {
                false
            };

            CommitteeMachine::<T>::insert(&who, committee_machine);
            MachineCommittee::<T>::insert(&machine_id, machine_committee);
            CommitteeOps::<T>::insert(&who, &machine_id, machine_ops);

            Ok(().into())
        }
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
        UndoChill(T::AccountId),
        Withdrawn(T::AccountId, BalanceOf<T>),
        AddToWhiteList(T::AccountId),
        AddToBlackList(T::AccountId),
        ExitFromCandidacy(T::AccountId),
        CommitteeFulfill(BalanceOf<T>),
        AddConfirmHash(T::AccountId, [u8; 16]),
        RmFromBlackList(T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        AccountAlreadyExist,
        AccountNotExist,
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
        StatusNotAllowedSubmitHash,
        NotAllHashSubmited,
        HashIsNotIdentical,
        NoMachineCanBook,
        NoMachineIdFound,
        JobNotDone,
        NotHealthStatus,
        NoNeedFulfill,
        NoLedgerFound,
        MinStakeNotFound,
        NotInChillList,
        TimeNotAllow,
        NotSubmitHash,
        AlreadySubmitRaw,
    }
}

// #[rustfmt::skip]
impl<T: Config> Pallet<T> {
    // TODO: 实现machine_info hash
    // fn get_machine_info_hash(machine_info: MachineInfoByCommittee) -> [u8; 16] {
    //     let a: [u8; 16] = [0; 16];
    //     return a;
    // }

    // fn hash_is_identical(raw_input: &Vec<u8>, hash: [u8; 16]) -> bool {
    //     let raw_hash: [u8; 16] = blake2_128(raw_input);
    //     return raw_hash == hash;
    // }

    // 根据DBC价格获得最小质押数量
    fn get_min_stake_amount() -> Option<BalanceOf<T>> {
        let dbc_price = <dbc_price_ocw::Module<T>>::avg_price();
        if let None = dbc_price {
            return None;
        }
        let dbc_price = dbc_price.unwrap();
        let committee_min_stake = Self::committee_min_stake();

        return Some((committee_min_stake / dbc_price).saturated_into());
    }

    // 获取所有新加入的机器，并进行分派给委员会
    fn distribute_machines() {
        let live_machines = <online_profile::Pallet<T>>::live_machines();
        for a_machine_id in live_machines.ocw_confirmed_machine {
            Self::distribute_one_machine(&a_machine_id);
        }
    }

    fn distribute_one_machine(machine_id: &MachineId) {
        let lucky_committee = Self::lucky_committee();
        if lucky_committee.len() == 0 {
            return;
        }

        // 每个添加4个小时
        let now = <frame_system::Module<T>>::block_number();
        let start_time: Vec<_> = (0..DISTRIBUTION)
            .map(|x| now + (x * 3600u32 / 30 * 4).into())
            .collect();

        // 修改机器的操作历史信息：记录分配的委员会及开始时间
        // TODO: 有优化点
        for (a_committee, &start_time) in lucky_committee.iter().zip(start_time.iter()) {
            let mut committee_ops = Self::committee_ops(&a_committee, &machine_id);
            committee_ops.booked_time = now;
            committee_ops.verify_time.push(start_time);
            CommitteeOps::<T>::insert(a_committee, machine_id, committee_ops);
        }

        // 修改机器对应的委员会
        let mut machine_committee = Self::machine_committee(machine_id);
        machine_committee.book_time = now;
        machine_committee.confirm_start = now + (3600u32 / 30 * 36).into(); // 添加确认信息时间为分发之后的36小时
        for a_committee in lucky_committee.clone().into_iter() {
            // 记录该机器分配的委员会到booked_committee
            if let Err(index) = machine_committee
                .booked_committee
                .binary_search(&a_committee)
            {
                machine_committee
                    .booked_committee
                    .insert(index, a_committee.clone());
            }

            // 修改委员会的机器, 记录委员会分配了哪些机器
            // TODO: 有优化点
            let mut committee_machine = Self::committee_machine(a_committee.clone());
            if let Err(index) = committee_machine.booked_machine.binary_search(&machine_id) {
                committee_machine
                    .booked_machine
                    .insert(index, machine_id.to_vec());
            }
            CommitteeMachine::<T>::insert(a_committee, committee_machine);
        }

        MachineCommittee::<T>::insert(machine_id, machine_committee);

        // 将机器状态从ocw_confirmed_machine改为booked_machine
        T::LCOperations::lc_booked_machine(machine_id.clone()); // 最后一步执行这个
    }

    // 分派一个machineId给随机的委员会
    // 返回Distribution(9)个随机顺序的账户列表
    fn lucky_committee() -> Vec<T::AccountId> {
        let staker = Self::committee();
        let mut verify_schedule = Vec::new();

        // 如果委员会数量为0，直接返回空列表
        if staker.committee.len() == 0 {
            return verify_schedule;
        }

        // 每n个区间，委员会循环一次, 这个区间n最大为3，最小为committee.len()
        let lucky_committee_len = if staker.committee.len() < 3 {
            staker.committee.len()
        } else {
            3
        };

        // 选出lucky_committee_len个委员会
        let mut committee = staker.committee.clone();
        let mut lucky_committee = Vec::new();
        for _ in 0..lucky_committee_len {
            let lucky_index =
                <random_num::Module<T>>::random_u32(committee.len() as u32 - 1u32) as usize;
            lucky_committee.push(committee[lucky_index].clone());
            committee.remove(lucky_index);
        }

        let repeat_slot = DISTRIBUTION as usize / lucky_committee_len;
        let extra_slot = DISTRIBUTION as usize % lucky_committee_len;

        for _ in 0..repeat_slot {
            let mut committee = lucky_committee.clone();
            for _ in 0..committee.len() {
                let lucky =
                    <random_num::Module<T>>::random_u32(committee.len() as u32 - 1u32) as usize;
                verify_schedule.push(committee[lucky].clone());
                committee.remove(lucky);
            }
        }

        for _ in 0..extra_slot {
            let mut committee = lucky_committee.clone();
            let lucky = <random_num::Module<T>>::random_u32(committee.len() as u32 - 1u32) as usize;
            verify_schedule.push(committee[lucky].clone());
            committee.remove(lucky);
        }

        verify_schedule
    }

    fn statistic_result() {
        let live_machines = <online_profile::Pallet<T>>::live_machines();
        let booked_machine = live_machines.booked_machine;
        let now = <frame_system::Module<T>>::block_number();

        for machine_id in booked_machine {
            // 如果机器超过了48个小时，则查看：
            // 是否有委员会提交了确认信息
            let machine_committee = Self::machine_committee(machine_id.clone());
            if now < machine_committee.book_time + (3600u32 / 30 * 48).into() {
                return;
            }

            let (support, against) = Self::summary_confirmation(&machine_id);

            // 没有委员会添加确认信息，或者意见相反委员会相等, 则进行新一轮评估
            if support == against {
                Self::revert_book(machine_id.clone());
                return;
            } else if support > against {
                // TODO: 机器被成功添加, 则添加上可以获取收益的委员会
            } else {
                // TODO: 机器没有被成功添加，拒绝这个机器，
            }
        }
    }

    // 重新进行派单评估
    // 该函数将清除本模块信息，并将online_profile机器状态改为ocw_confirmed_machine
    // 清除信息： LCCommitteeMachineList, LCMachineCommitteeList, LCCommitteeOps
    fn revert_book(machine_id: MachineId) {
        T::LCOperations::lc_revert_booked_machine(machine_id.clone());

        let mut machine_committee = Self::machine_committee(&machine_id);
        for booked_committee in machine_committee.booked_committee {
            OpsDetail::<T>::remove(&booked_committee, &machine_id);

            let mut committee_machine = Self::committee_machine();
            if let Ok(index) = committee_machine.booked_machine.binary_search(&machine_id) {
                committee_machine.booked_machine.remove(index);
            }
            if let Ok(index) = committee_machine.hashed_machine.binary_search(&machine_id) {
                committee_machine.hashed_machine.remove(index);
            }
            if let Ok(index) = committee_machine
                .confirmed_machine
                .binary_search(&machine_id)
            {
                committee_machine.confirmed_machine.remove(index);
            }
            CommitteeMachine::<T>::insert(booked_committee, committee_machine);
        }

        MachineCommittee::<T>::remove(&machine_id);
    }

    // 总结机器的确认情况
    fn summary_confirmation(machine_id: &MachineId) -> (u32, u32) {
        let machine_committee = Self::machine_committee(machine_id);
        let mut support = 0u32;
        let mut against = 0u32;

        if machine_committee.confirmed_committee.len() > 0 {
            for a_committee in machine_committee.confirmed_committee {
                let committee_ops = Self::committee_ops(a_committee, machine_id);
                if committee_ops.confirm_result == true {
                    support += 1;
                } else {
                    against += 1;
                }
            }
        }

        return (support, against);
    }

    fn update_ledger(controller: &T::AccountId, ledger: &StakingLedger<BalanceOf<T>>) {
        <T as Config>::Currency::set_lock(
            PALLET_LOCK_ID,
            controller,
            ledger.total,
            WithdrawReasons::all(),
        );
        <CommitteeLedger<T>>::insert(controller, Some(ledger));
    }
}
