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
use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;
use sp_std::{prelude::*, str, vec::Vec};

pub use pallet::*;

pub type MachineId = Vec<u8>;
pub type OrderId = u64; // 提交的单据ID
                        // 机器故障原因
pub enum ReportReason {}

// 记录该模块中活跃的订单
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct LiveOrder {
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
    pub report_time: BlockNumber,                        // 机器被报告时间
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
    pub trait Config: frame_system::Config {}

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

        #[pallet::weight(10000)]
        pub fn report_machine_state(origin: OriginFor<T>, raw_hash: Vec<u8>) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            // TODO: 增加确认用户是否有资格报告机器状态

            Ok(().into())
        }
    }
}

// #[rustfmt::skip]
impl<T: Config> Pallet<T> {}
