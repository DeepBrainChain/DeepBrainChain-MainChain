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
    pallet_prelude::*,
    traits::{Currency, LockIdentifier, LockableCurrency, WithdrawReasons},
};
use frame_system::pallet_prelude::*;
use sp_io::hashing::blake2_128;
use sp_runtime::{
    traits::{CheckedDiv, CheckedMul, SaturatedConversion},
    RuntimeDebug,
};
use sp_std::{prelude::*, str, vec::Vec};

pub use pallet::*;

pub type MachineId = Vec<u8>;
pub type OrderId = u64; // 提交的单据ID
pub type Pubkey = Vec<u8>;
pub type Hash = [u8; 16];
type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub const PALLET_LOCK_ID: LockIdentifier = *b"mtcommit";

// 记录该模块中活跃的订单
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct LiveOrderList {
    pub reported_order: Vec<OrderId>,         // 委员会还可以抢单的订单
    pub fully_order: Vec<OrderId>,            // 已经被抢完的机器ID，不能再进行抢单
    pub fully_reporter_hashed: Vec<OrderId>,  // reporter已经全部提交了Hash的机器ID
    pub fully_committee_hashed: Vec<OrderId>, // 委员会已经提交了全部Hash的机器Id
    pub fully_raw: Vec<OrderId>,              // 已经全部提交了Raw的机器Id
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct ReporterRecord {
    pub reported_id: Vec<OrderId>,
}

// 记录处于不同状态的委员会的列表，方便派单
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct StakerList<AccountId: Ord> {
    pub committee: Vec<AccountId>,    // 质押并通过社区选举的委员会
    pub pubkey_list: Vec<AccountId>,  // 委员会，但需要提交公钥，新加入的委员会需要首先添加pub_key
    pub fulfill_list: Vec<AccountId>, // 委员会, 但需要补交质押
    pub chill_list: Vec<AccountId>,   // 委员会，但不想被派单
    pub black_list: Vec<AccountId>,   // 委员会，黑名单中
}

impl<AccountId: Ord> StakerList<AccountId> {
    fn staker_exist(&self, who: &AccountId) -> bool {
        if let Ok(_) = self.committee.binary_search(who) {
            return true;
        }
        if let Ok(_) = self.pubkey_list.binary_search(who) {
            return true;
        }
        if let Ok(_) = self.fulfill_list.binary_search(who) {
            return true;
        }
        if let Ok(_) = self.chill_list.binary_search(who) {
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
pub struct CommitteeMachineList {
    pub booked_order: Vec<OrderId>, // 记录分配给用户的订单及开始验证时间
    pub hashed_order: Vec<OrderId>, // 存储已经提交了Hash信息的订单
    pub confirmed_order: Vec<OrderId>, // 存储已经提交了原始确认数据的订单
    pub online_machine: Vec<MachineId>, // 存储已经成功上线的机器
}

// 一台机器对应的委员会
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineCommitteeList<AccountId, BlockNumber> {
    pub order_id: OrderId,
    pub report_time: BlockNumber,         // 机器被报告时间
    pub booked_committee: Vec<AccountId>, // 记录分配给机器的委员会及验证开始时间
    pub hashed_committee: Vec<AccountId>,
    pub confirm_start: BlockNumber, // 开始提交raw信息的时间
    pub confirmed_committee: Vec<AccountId>,
    pub onlined_committee: Vec<AccountId>,
}

// 委员会对机器的操作信息
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeMachineOps<BlockNumber> {
    pub booked_time: BlockNumber,
    pub encrypted_err_info: Option<Vec<u8>>, // reporter 提交的加密后的信息
    pub encrypted_login_info: Option<Vec<u8>>, // 记录机器的登录信息，用委员会公钥加密
    pub encrypted_time: BlockNumber,
    pub confirm_hash: Hash, // TODO:
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

    // 默认抢单委员会的个数
    #[pallet::type_value]
    pub fn CommitteeLimitDefault<T: Config> () -> u32 {
        3
    }

    // 最多多少个委员会能够抢单
    #[pallet::storage]
    #[pallet::getter(fn committee_limit)]
    pub(super) type CommitteeLimit<T: Config> = StorageValue<_, u32, ValueQuery, CommitteeLimitDefault<T>>;

    // 委员会的公钥信息
    #[pallet::storage]
    #[pallet::getter(fn committee_pubkey)]
    pub(super) type CommitteePubkey<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Pubkey, ValueQuery>;

    // 查询报告人报告的机器
    #[pallet::storage]
    #[pallet::getter(fn reporter_order)]
    pub(super) type ReporterOrder<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, ReporterRecord, ValueQuery>;

    // 通过报告单据ID，查询报告的机器的信息(委员会抢单信息)
    #[pallet::storage]
    #[pallet::getter(fn reported_machines)]
    pub(super) type ReportedMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, OrderId, MachineCommitteeList<T::AccountId, T::BlockNumber>, ValueQuery>;

    // 委员会查询自己的抢单信息
    #[pallet::storage]
    #[pallet::getter(fn committee_machines)]
    pub(super) type CommitteeMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, CommitteeMachineList, ValueQuery>;

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

            // 将用户添加到pubkey_list列表中
            StakerList::add_staker(&mut staker.pubkey_list, member.clone());
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
        // 用户报告机器有问题
        #[pallet::weight(10000)]
        pub fn report_machine_state(origin: OriginFor<T>, _raw_hash: Vec<u8>) -> DispatchResultWithPostInfo {
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

            // 记录到报告人的存储中
            let mut reporter_order = Self::reporter_order(&reporter);
            if let Err(index) = reporter_order.reported_id.binary_search(&order_id) {
                reporter_order.reported_id.insert(index, order_id);
            }
            ReporterOrder::<T>::insert(&reporter, reporter_order);

            // 更新NextOrderId
            NextOrderId::<T>::put(order_id + 1);

            Ok(().into())
        }

        // 委员会进行抢单
        #[pallet::weight(10000)]
        pub fn book_one(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            // 判断发起请求者是状态正常的委员会
            let staker = Self::staker();
            if let Err(_) = staker.committee.binary_search(&who) {
                return Err(Error::<T>::NotCommittee.into());
            }

            // 检查是否有可预订的订单
            let mut live_order = Self::live_order();
            ensure!(live_order.reported_order.len() > 0, Error::<T>::NoBookableOrder);

            // 从live_order取出一个
            let one_order = live_order.reported_order.pop().unwrap();

            // 添加到委员会自己的存储中
            let mut committee_booked = Self::committee_machines(&who);
            if let Err(index) = committee_booked.booked_order.binary_search(&one_order) {
                committee_booked.booked_order.insert(index, one_order);
            }

            // 判断是否已经有三个委员会抢单，如果满足，则将订单放到fully_order中
            // 如果委员会抢单的个数大于等于限制，则停止抢单，并改变live_order存储
            let mut reported_machines = Self::reported_machines(one_order);
            if let Err(index) = reported_machines.booked_committee.binary_search(&who) {
                reported_machines.booked_committee.insert(index, who.clone());
            }

            let committee_limit = Self::committee_limit();
            if reported_machines.booked_committee.len() >= committee_limit as usize {
                if let Ok(index) = live_order.reported_order.binary_search(&one_order) {
                    live_order.reported_order.remove(index);
                }

                if let Err(index) = live_order.fully_order.binary_search(&one_order) {
                    live_order.fully_order.insert(index, one_order);
                }

                LiveOrder::<T>::put(live_order);
            }

            // 添加委员会对于机器的操作记录
            let mut ops_detail = Self::committee_ops(&who, &one_order);
            ops_detail.booked_time = now;

            // 修改存储
            CommitteeMachines::<T>::insert(&who, committee_booked);
            ReportedMachines::<T>::insert(&one_order, reported_machines);
            CommitteeOps::<T>::insert(&who, &one_order, ops_detail);

            Ok(().into())
        }

        // 委员会任何时候都可以添加公钥信息，添加公钥信息不应改变委员会的状态。
        // 但是，当委员会在pubkey_list列表中时，为刚成为委员会的状态。添加pubkey则变为fulfill_list状态
        #[pallet::weight(10000)]
        pub fn reporter_add_pubkey(origin: OriginFor<T>, pubkey: Pubkey) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;

            // 检查是否为委员会
            let mut staker = Self::staker();
            ensure!(staker.staker_exist(&committee), Error::<T>::NotCommittee);

            // 添加pubkey
            CommitteePubkey::<T>::insert(&committee, pubkey.clone());

            // 检查是否在pubkey_list中， 如果在则改变committee的状态
            if let Ok(_) = staker.pubkey_list.binary_search(&committee) {
                StakerList::rm_staker(&mut staker.pubkey_list, &committee);
                StakerList::add_staker(&mut staker.fulfill_list, committee.clone());
                Staker::<T>::put(staker);
            }

            Self::deposit_event(Event::CommitteeAddPubkey(pubkey));

            Ok(().into())
        }

        // FIXME: 完善逻辑
        // 报告人在委员会完成抢单后，24小时内用委员会的公钥，提交加密后的故障信息
        #[pallet::weight(10000)]
        pub fn reporter_add_error_hash(origin: OriginFor<T>, order_id: OrderId, to_committee: T::AccountId, encrypted_err_info: Vec<u8>, encrypted_login_info: Vec<u8>) -> DispatchResultWithPostInfo {
            let _reporter = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            // 检查该用户为order_id的reporter
            let reporter_order = Self::reporter_order(&to_committee);
            if let Err(_) = reporter_order.reported_id.binary_search(&order_id) {
                return Err(Error::<T>::NotOrderReporter.into());
            }

            // 且该order_id处于可提提交的状态
            let mut live_order = Self::live_order();
            if let Err(_) = live_order.fully_order.binary_search(&order_id) {
                return Err(Error::<T>::OrderStatusNotFeat.into());
            }

            // 检查该委员会为预订了该订单的委员会
            let machine_committee = Self::reported_machines(&order_id);
            if let Err(_) = machine_committee.booked_committee.binary_search(&to_committee) {
                return Err(Error::<T>::NotOrderCommittee.into())
            }

            // 添加到委员会对机器的信息中
            let mut committee_ops = Self::committee_ops(&to_committee, &order_id);
            if let None = committee_ops.encrypted_err_info {
                committee_ops.encrypted_err_info = Some(encrypted_err_info);
                committee_ops.encrypted_login_info = Some(encrypted_login_info);
                committee_ops.encrypted_time = now;
                CommitteeOps::<T>::insert(&to_committee, &order_id, committee_ops);
            } else {
                return Err(Error::<T>::AlreadySubmitEncryptedInfo.into());
            }

            // 检查是否为所有委员会提交了信息
            for _a_committee in machine_committee.booked_committee.iter() {
                let committee_ops = Self::committee_ops(&to_committee, &order_id);
                if let None = committee_ops.encrypted_err_info {
                    // 还有未提供加密信息的委员会
                    return Ok(().into())
                }
            }

            // 所有加密信息都已提供
            if let Ok(index) = live_order.fully_order.binary_search(&order_id) {
                live_order.fully_order.remove(index);
            }
            if let Err(index) = live_order.fully_reporter_hashed.binary_search(&order_id) {
                live_order.fully_reporter_hashed.insert(index, order_id)
            }

            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn add_confirm_hash(origin: OriginFor<T>, order_id: OrderId, hash: Hash) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            // 判断是否为委员会其列表是否有该order_id
            let committee_booked = Self::committee_machines(&committee);
            if let Err(_) = committee_booked.booked_order.binary_search(&order_id) {
                return Err(Error::<T>::NotInBookedList.into());
            }

            // 判断该order_id是否可以提交信息
            let live_order = Self::live_order();
            if let Err(_) = live_order.fully_order.binary_search(&order_id) {
                return Err(Error::<T>::OrderStatusNotFeat.into())
            }

            // 判断时间是否允许提交记录
            let mut ops_detail = Self::committee_ops(&committee, &order_id);

            // 允许之后，添加到存储中
            ops_detail.confirm_hash = hash;
            ops_detail.hash_time = now;
            CommitteeOps::<T>::insert(&committee, &order_id, ops_detail);

            // 判断是否已经全部提交了Hash，如果是，则改变该订单状态
            let mut reported_machines = Self::reported_machines(&order_id);
            if let Err(index) = reported_machines.hashed_committee.binary_search(&committee) {
                reported_machines.hashed_committee.insert(index, committee.clone());
            }

            if reported_machines.hashed_committee.len() == reported_machines.booked_committee.len() {
                let mut committee_machine = Self::committee_machines(&committee);

                if let Ok(index) = committee_machine.booked_order.binary_search(&order_id) {
                    committee_machine.booked_order.remove(index);
                }

                if let Err(index) = committee_machine.hashed_order.binary_search(&order_id) {
                    committee_machine.hashed_order.insert(index, order_id);
                }

                CommitteeMachines::<T>::insert(&committee, committee_machine);
            }

            ReportedMachines::<T>::insert(&order_id, reported_machines);
            Ok(().into())
        }

        #[pallet::weight(10000)]
        fn submit_confirm_raw(origin: OriginFor<T>, _machine_id: MachineId, _reporter_rand_str: Vec<u8>, _committee_rand_str: Vec<u8>, _err_type: Vec<u8>) -> DispatchResultWithPostInfo {
            let _committee = ensure_signed(origin)?;
            // 1. 判断Hash = 报告人提交的原始Hash

            // 2. 根据委员会的统计，判断是否有故障，并更新故障信息到online_profile

            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        CommitteeAdded(T::AccountId),
        CommitteeFulfill(BalanceOf<T>),
        CommitteeAddPubkey(Pubkey),
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
        NoBookableOrder,
        OrderStatusNotFeat,
        NotInBookedList,
        AlreadySubmitEncryptedInfo,
        NotOrderReporter,
        NotOrderCommittee,
    }
}

// #[rustfmt::skip]
impl<T: Config> Pallet<T> {
    // 根据DBC价格获得最小质押数量
    // DBC精度15，Balance为u128, min_stake不超过10^24 usd 不会超出最大值
    fn get_min_stake_amount() -> Option<BalanceOf<T>> {
        let one_dbc: BalanceOf<T> = 1000_000_000_000_000u64.saturated_into();

        let dbc_price = <dbc_price_ocw::Module<T>>::avg_price();
        if let None = dbc_price {
            return None;
        }
        let dbc_price = dbc_price.unwrap();
        let committee_min_stake = Self::committee_min_stake();

        // dbc_need = one_dbc * committee_min_stake / dbc_price
        let min_stake = one_dbc.checked_mul(&committee_min_stake.saturated_into::<BalanceOf<T>>());
        if let Some(_) = min_stake {
            return None;
        }
        let min_stake = min_stake.unwrap();
        min_stake.checked_div(&dbc_price.saturated_into::<BalanceOf<T>>())
    }

    fn _get_hash(raw_str: &Vec<u8>) -> [u8; 16] {
        return blake2_128(raw_str);
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
