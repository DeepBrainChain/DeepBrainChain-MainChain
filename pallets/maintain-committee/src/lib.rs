// 机器维护说明：
// 1. 机器空闲时，报告人无法报告。机器拥有者可以主动下线
// 2. 机器正在使用中，或者无法租用时，由报告人去报告。走本模块的报告--委员会审查流程。
//
// 具体流程：
// 1. 报告人提交Hash1, Hash1 = Hash(machineId, 随机字符串, 故障原因)
// 2. 委员会抢单。允许3个委员会抢单。委员会抢单后，报告人必须在24小时内，使用抢单委员会的公钥，提交加密后的信息：
//      upload(committee_id, Hash2); 其中, Hash2 = public_key(machineId, 随机字符串, 故障原因)
// 3. 委员会看到提交信息之后,使用自己的私钥,获取到报告人的信息,并需要**立即**去验证机器是否有问题。验证完则提交加密信息: Hash3
//    Hash3 = Hash(machineId, 报告人随机字符串，自己随机字符串，故障原因, 自己是否认可有故障)
// 4. 三个委员会都提交完信息之后，3小时后，提交原始信息： machineId, 报告人随机字符串，自己的随机字符串, 故障原因
//    需要： a. 判断Hash(machineId, 报告人随机字符串, 故障原因) ？= 报告人Hash
//          b. 根据委员会的统计，最终确定是否有故障。
// 5. 信息提交后，若有问题，直接扣除14天剩余奖励，若24小时，机器管理者仍未提交“机器已修复”，则扣除所有奖励。

#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode, HasCompact};
use frame_support::{
    dispatch::DispatchResult,
    pallet_prelude::*,
    traits::{Currency, LockIdentifier, LockableCurrency, WithdrawReasons},
};
use frame_system::pallet_prelude::*;
use sp_runtime::{traits::SaturatedConversion, RuntimeDebug};
use sp_std::{prelude::*, str, vec::Vec};

pub use pallet::*;

pub type MachineId = Vec<u8>;
pub type OrderId = u64; // 提交的单据ID
type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub const PALLET_LOCK_ID: LockIdentifier = *b"mtcommit";

// 机器故障原因
pub enum ReportReason {}

// 记录该模块中活跃的订单
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct LiveOrderList {
    pub reported_order: Vec<OrderId>, // 委员会还可以抢单的订单
    pub fully_order: Vec<OrderId>,    // 已经被抢完的机器ID，不能再进行抢单
}

// 记录处于不同状态的委员会的列表，方便派单
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct StakerList<AccountId: Ord> {
    pub committee: Vec<AccountId>,    // 质押并通过社区选举的委员会
    pub chill_list: Vec<AccountId>,   // 委员会，但不想被派单
    pub fulfill_list: Vec<AccountId>, // 委员会, 但需要补交质押
    pub black_list: Vec<AccountId>,   // 委员会，黑名单中
}

impl<AccountId: Ord> StakerList<AccountId> {
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
pub struct CommitteeMachineList<BlockNumber> {
    pub booked_order: Vec<(OrderId, BlockNumber)>, // 记录分配给用户的订单及开始验证时间
    pub hashed_order: Vec<OrderId>,                // 存储已经提交了Hash信息的订单
    pub confirmed_order: Vec<OrderId>,             // 存储已经提交了原始确认数据的订单
    pub online_machine: Vec<MachineId>,            // 存储已经成功上线的机器
}

// 一台机器对应的委员会
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineCommitteeList<AccountId, BlockNumber> {
    pub order_id: OrderId,
    pub report_time: BlockNumber, // 机器被报告时间
    pub booked_committee: Vec<(AccountId, BlockNumber)>, // 记录分配给机器的委员会及验证开始时间
    pub hashed_committee: Vec<AccountId>,
    pub confirm_start: BlockNumber, // 开始提交raw信息的时间
    pub confirmed_committee: Vec<AccountId>,
    pub onlined_committee: Vec<AccountId>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeMachineOps<BlockNumber> {
    pub booked_time: BlockNumber,
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
    pub trait Config: frame_system::Config + dbc_price_ocw::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    // 委员会最小质押, 默认100RMB等值DBC
    #[pallet::storage]
    #[pallet::getter(fn committee_min_stake)]
    pub(super) type CommitteeMinStake<T: Config> = StorageValue<_, u64, ValueQuery>;

    // 报告人最小质押，默认100RMB等值DBC
    #[pallet::storage]
    #[pallet::getter(fn reporter_min_stake)]
    pub(super) type ReporterMinStake<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn staker)]
    pub(super) type Staker<T: Config> = StorageValue<_, StakerList<T::AccountId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn committee_ledger)]
    pub(super) type CommitteeLedger<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Option<StakingLedger<BalanceOf<T>>>, ValueQuery>;

    // 通过报告单据ID，查询报告的机器的信息(委员会抢单信息)
    #[pallet::storage]
    #[pallet::getter(fn reported_machines)]
    pub(super) type ReportedMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, OrderId, MachineCommitteeList<T::AccountId, T::BlockNumber>, ValueQuery>;

    // 委员会查询自己的抢单信息
    #[pallet::storage]
    #[pallet::getter(fn committee_machines)]
    pub(super) type CommitteeMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, CommitteeMachineList<T::BlockNumber>, ValueQuery>;

    // 存储委员会对单台机器的操作记录
    #[pallet::storage]
    #[pallet::getter(fn committee_ops)]
    pub(super) type CommitteeOps<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, OrderId, CommitteeMachineOps<T::BlockNumber>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn live_order)]
    pub(super) type LiveOrder<T: Config> = StorageValue<_, LiveOrderList, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_order_id)]
    pub(super) type NextOrderId<T: Config> = StorageValue<_, OrderId, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置委员会的最小质押，单位： usd * 10^6
        #[pallet::weight(0)]
        pub fn set_committee_min_stake(origin: OriginFor<T>, value: u64) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            CommitteeMinStake::<T>::put(value);
            Ok(().into())
        }

        // 设置报告人最小质押，单位：usd * 10^6
        #[pallet::weight(0)]
        pub fn set_reporter_min_stake(origin: OriginFor<T>, value: u64) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            ReporterMinStake::<T>::put(value);
            Ok(().into())
        }

        // 该操作由社区决定
        // Root权限，添加到委员会，直接添加到fulfill列表中。当竞选成功后，需要操作以从fulfill_list到committee
        #[pallet::weight(0)]
        pub fn add_committee(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut staker = Self::staker();

            // 确保用户还未加入到本模块
            ensure!(!staker.staker_exist(&member), Error::<T>::AccountAlreadyExist);

            // 将用户添加到fulfill列表中
            StakerList::add_staker(&mut staker.fulfill_list, member.clone());
            Self::deposit_event(Event::CommitteeAdded(member));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn fill_pledge(origin: OriginFor<T>) -> DispatchResultWithPostInfo{
            let who = ensure_signed(origin)?;

            // 检查是否在fulfill列表中
            let mut staker = Self::staker();
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
            StakerList::rm_staker(&mut staker.fulfill_list, &who);
            StakerList::add_staker(&mut staker.committee, who.clone());

            Self::deposit_event(Event::CommitteeFulfill(needed));

            Ok(().into())
        }

        // 委员会停止接单
        #[pallet::weight(10000)]
        pub fn chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut staker = Self::staker();
            ensure!(staker.staker_exist(&who), Error::<T>::AccountNotExist);

            // 只有committee状态才允许进行chill
            if let Err(_) = staker.committee.binary_search(&who) {
                return Err(Error::<T>::NotCommittee.into());
            }

            StakerList::rm_staker(&mut staker.committee, &who);
            StakerList::add_staker(&mut staker.chill_list, who.clone());

            Staker::<T>::put(staker);
            Self::deposit_event(Event::Chill(who));

            Ok(().into())
        }

        // 委员会可以接单
        #[pallet::weight(10000)]
        pub fn undo_chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut staker = Self::staker();
            if let Err(_) = staker.chill_list.binary_search(&who) {
                return Err(Error::<T>::NotInChillList.into());
            }

            StakerList::rm_staker(&mut staker.chill_list, &who);
            StakerList::add_staker(&mut staker.committee, who.clone());
            Staker::<T>::put(staker);

            Self::deposit_event(Event::UndoChill(who));
            Ok(().into())
        }

        // 委员会可以退出, 从chill_list中退出
        #[pallet::weight(10000)]
        pub fn exit_staker(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut staker = Self::staker();
            ensure!(staker.staker_exist(&who), Error::<T>::AccountNotExist);

            // 如果有未完成的工作，则不允许退出
            let committee_machines = Self::committee_machines(&who);
            if committee_machines.booked_order.len() > 0 ||
                committee_machines.hashed_order.len() > 0 ||
                committee_machines.confirmed_order.len() > 0 {
                    return Err(Error::<T>::JobNotDone.into());
            }

            // 如果是candidacy，则可以直接退出, 从staker中删除
            // 如果是fulfill_list则可以直接退出(低于5wDBC的将进入fulfill_list，无法抢单,每次惩罚1w)
            StakerList::rm_staker(&mut staker.committee, &who);
            StakerList::rm_staker(&mut staker.fulfill_list, &who);
            StakerList::rm_staker(&mut staker.chill_list, &who);

            Staker::<T>::put(staker);
            let ledger = Self::committee_ledger(&who);
            if let Some(mut ledger) = ledger {
                ledger.total = 0u32.into();
                Self::update_ledger(&who, &ledger);
            }

            CommitteeLedger::<T>::remove(&who);
            Self::deposit_event(Event::ExitFromCandidacy(who));

            return Ok(().into());
        }

        // FIXME: 必须特定的用户才能进行报告。避免用户报告同一台机器多次
        #[pallet::weight(10000)]
        pub fn report_machine_state(origin: OriginFor<T>, raw_hash: Vec<u8>) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            // 被报告的机器存储起来，委员会进行抢单
            let mut live_order = Self::live_order();
            let order_id = Self::next_order_id();
            if let Err(index) = live_order.reported_order.binary_search(&order_id) {
                live_order.reported_order.insert(index, order_id);
            }
            LiveOrder::<T>::put(live_order);

            // let mut machine_committee = Self::machine_committee(&next_order_id);

            ReportedMachines::<T>::insert(&order_id, MachineCommitteeList {
                order_id: order_id,
                report_time: now,
                ..Default::default()
            });

            // 更新NextOrderId
            NextOrderId::<T>::put(order_id + 1);

            Ok(().into())
        }

        // 委员会进行抢单
        #[pallet::weight(10000)]
        pub fn book_order(origin: OriginFor<T>, order_id: OrderId) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            // TODO: call book_one_order

            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn book_one(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            // TODO: call book_one_order

            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn add_confirm_hash(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;

            Ok(().into())
        }

        #[pallet::weight(10000)]
        fn submit_confirm_raw(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            ensure_signed(origin)?;

            Ok(().into())
        }
    }


    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        CommitteeAdded(T::AccountId),
        CommitteeFulfill(BalanceOf<T>),
        Chill(T::AccountId),
        ExitFromCandidacy(T::AccountId),
        UndoChill(T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        NotCommittee,
        MaxBookReached,
        AlreadyBooked,
        AccountAlreadyExist,
        NoNeedFulfill,
        MinStakeNotFound,
        FreeBalanceNotEnough,
        AccountNotExist,
        NotInChillList,
        JobNotDone,
    }
}

// #[rustfmt::skip]
impl<T: Config> Pallet<T> {
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

    fn book_one_order(who: T::AccountId, order_id: OrderId) -> DispatchResult {
        // 检查是否是委员会
        let staker = Self::staker();
        if let Err(_) = staker.committee.binary_search(&who) {
            Err(Error::<T>::NotCommittee)?
        }

        // 检查是否达到了最大预订数量
        let mut reported_machines = Self::reported_machines(&order_id);
        if reported_machines.booked_committee.len() >= 3 {
            Err(Error::<T>::MaxBookReached)?
        }

        // 检查该委员会是否已经预订过该订单
        let ordered_committee = reported_machines
            .booked_committee
            .iter()
            .filter(|x| x.0 == who)
            .collect::<Vec<_>>();
        if ordered_committee.len() != reported_machines.booked_committee.len() {
            Err(Error::<T>::AlreadyBooked)?
        }

        let now = <frame_system::Module<T>>::block_number();

        let mut committee_machines = Self::committee_machines(&who);
        let mut ops_detail = Self::committee_ops(&who, &order_id);

        reported_machines.booked_committee.push((who.clone(), now));
        committee_machines.booked_order.push((order_id, now));
        ops_detail.booked_time = now;

        // 如果预订的委员会达到3位，则将订单id移动到fully_order中
        if reported_machines.booked_committee.len() >= 3 {
            let mut live_order = Self::live_order();

            if let Ok(index) = live_order.reported_order.binary_search(&order_id) {
                live_order.reported_order.remove(index);
            }
            if let Err(index) = live_order.fully_order.binary_search(&order_id) {
                live_order.fully_order.insert(index, order_id);
            }

            LiveOrder::<T>::put(live_order);
        }

        CommitteeMachines::<T>::insert(&who, committee_machines);
        CommitteeOps::<T>::insert(&who, &order_id, ops_detail);
        ReportedMachines::<T>::insert(&order_id, reported_machines);

        Ok(())
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
