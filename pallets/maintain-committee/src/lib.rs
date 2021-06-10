// 机器维护说明：
// 1. 机器空闲时，报告人无法报告。机器拥有者可以主动下线
// 2. 机器正在使用中，或者无法租用时，由报告人去报告。走本模块的报告--委员会审查流程。
//
// 具体流程：
// 1. 报告人提交Hash1, Hash1 = Hash(machineId, 随机字符串, 故障原因)
// 2. 委员会抢单。允许3个委员会抢单。委员会抢单后，报告人必须在24小时内，使用抢单委员会的公钥，提交加密后的信息：
//      upload(committee_id, Hash2); 其中, Hash2 = public_key(machineId, 随机字符串, 故障原因)
// 3. 委员会看到提交信息之后,使用自己的私钥,获取到报告人的信息,并需要**立即**去验证机器是否有问题。验证完则提交加密信息: Hash3
//    Hash3 = Hash(machineId, 报告人随机字符串，自己随机字符串，自己是否认可有故障, 故障原因)
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
    traits::{CheckedDiv, CheckedMul, CheckedSub, CheckedAdd, SaturatedConversion},
    RuntimeDebug,
};
use sp_std::{prelude::*, str, vec::Vec};

pub use pallet::*;

pub type MachineId = Vec<u8>;
pub type OrderId = u64; // 提交的单据ID
pub type Hash = [u8; 16];
type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub const PALLET_LOCK_ID: LockIdentifier = *b"mtcommit";

// 记录该模块中所有活跃的订单, 根据ReportStatus来划分
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct LiveOrderList {
    pub reported_order: Vec<OrderId>,         // 委员会还可以抢单的订单
    pub fully_order: Vec<OrderId>,            // 已经被抢完的机器ID，不能再进行抢单
    pub fully_report_encrypted_info: Vec<OrderId>,  // reporter已经全部提交了Hash的机器ID
    pub fully_committee_hashed: Vec<OrderId>, // 委员会已经提交了全部Hash的机器Id
    pub fully_raw: Vec<OrderId>,              // 已经全部提交了Raw的机器Id
}

// 报告人的订单
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct ReporterRecord {
    pub reported_id: Vec<OrderId>,
}

//  委员会的列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct StakerList<AccountId: Ord> {
    pub committee: Vec<AccountId>,    // 质押并通过社区选举的委员会
    pub waiting_box_pubkey: Vec<AccountId>,
    pub black_list: Vec<AccountId>,   // 委员会，黑名单中
}

// 委员会抢到的订单的列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeMachineList {
    pub booked_order: Vec<OrderId>, // 记录分配给用户的订单及开始验证时间
    pub hashed_order: Vec<OrderId>, // 存储已经提交了Hash信息的订单
    pub confirmed_order: Vec<OrderId>, // 存储已经提交了原始确认数据的订单
    pub online_machine: Vec<MachineId>, // 存储已经成功上线的机器
}

// 订单对应的委员会
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineCommitteeList<AccountId, BlockNumber, Balance> {
    pub order_id: OrderId,
    pub report_time: BlockNumber, // 机器被报告时间
    pub raw_hash: Hash, // 包含错误原因的hash
    pub box_public_key: [u8; 32], // 用户私钥生成的box_public_key，用于委员会解密
    pub reporter_stake: Balance,
    pub first_book_time: BlockNumber,
    pub machine_id: MachineId, // 只有委员会提交原始信息时才存入
    pub err_info: Vec<u8>,
    pub booked_committee: Vec<AccountId>, // 记录分配给机器的委员会及验证开始时间
    pub get_encrypted_info_committee: Vec<AccountId>, // 已经获得了加密信息的委员会列表
    pub hashed_committee: Vec<AccountId>,
    pub confirm_start: BlockNumber, // 开始提交raw信息的时间
    pub confirmed_committee: Vec<AccountId>,
    pub onlined_committee: Vec<AccountId>,
    pub machine_status: ReportStatus, // 记录当前订单的状态
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum ReportStatus {
    Reported, // 没有任何人预订过的订单, 允许取消
    WaitingBook, // 前一个委员会的订单已经超过一个小时，自动改成可预订状态
    Verifying, // 已经有委员会抢单，正处于验证中
    SubmitingRaw, // 已经到了3个小时，正在等待委员会上传原始信息
    Confirmed, // TODO: 委员会已经完成，等待48小时, 执行订单结果
}

impl Default for ReportStatus {
    fn default() -> Self {
        ReportStatus::Reported
    }
}

// 委员会对机器的操作信息
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct CommitteeMachineOps<BlockNumber, Balance> {
    pub booked_time: BlockNumber,
    pub encrypted_err_info: Option<Vec<u8>>, // reporter 提交的加密后的信息
    pub encrypted_time: BlockNumber,
    pub confirm_hash: Hash,
    pub hash_time: BlockNumber,
    pub confirm_raw: Vec<u8>,
    pub confirm_time: BlockNumber, // 委员会提交raw信息的时间
    pub confirm_result: bool,
    pub machine_status: OrderStatus, //TODO: 应该改为委员会自己的状态
    pub staked_balance: Balance,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum OrderStatus {
    Verifying, // 一旦预订订单，状态将是Verifying
    Hashed, // 提交了Hash之后的状态
    RawSubmited, // 已经提交了原始信息的状态
}

impl Default for OrderStatus {
    fn default() -> Self {
        OrderStatus::Verifying
    }
}

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
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_block_number: T::BlockNumber) {
            // 每个块检查状态是否需要变化。
            // 抢单逻辑不能在finalize中处理，防止一个块有多个抢单请求
            Self::heart_beat();
        }
    }

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

    // 默认抢单委员会的个数
    #[pallet::type_value]
    pub fn CommitteeLimitDefault<T: Config> () -> u32 {
        3
    }

    #[pallet::storage]
    #[pallet::getter(fn box_pubkey)]
    pub(super) type BoxPubkey<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, [u8; 32], ValueQuery>;

    // 存储每个用户在该模块中的总质押量
    #[pallet::storage]
    #[pallet::getter(fn user_total_stake)]
    pub(super) type UserTotalStake<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    // 最多多少个委员会能够抢单
    #[pallet::storage]
    #[pallet::getter(fn committee_limit)]
    pub(super) type CommitteeLimit<T: Config> = StorageValue<_, u32, ValueQuery, CommitteeLimitDefault<T>>;

    // 查询报告人报告的机器
    #[pallet::storage]
    #[pallet::getter(fn reporter_order)]
    pub(super) type ReporterOrder<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, ReporterRecord, ValueQuery>;

    // 通过报告单据ID，查询报告的机器的信息(委员会抢单信息)
    #[pallet::storage]
    #[pallet::getter(fn reported_order_info)]
    pub(super) type ReportedOrderInfo<T: Config> =
        StorageMap<_, Blake2_128Concat, OrderId, MachineCommitteeList<T::AccountId, T::BlockNumber, BalanceOf<T>>, ValueQuery>;

    // 委员会查询自己的抢单信息
    #[pallet::storage]
    #[pallet::getter(fn committee_machines)]
    pub(super) type CommitteeMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, CommitteeMachineList, ValueQuery>;

    // 存储委员会对单台机器的操作记录
    #[pallet::storage]
    #[pallet::getter(fn committee_ops)]
    pub(super) type CommitteeOps<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, OrderId, CommitteeMachineOps<T::BlockNumber, BalanceOf<T>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn live_order)]
    pub(super) type LiveOrder<T: Config> = StorageValue<_, LiveOrderList, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_order_id)]
    pub(super) type NextOrderId<T: Config> = StorageValue<_, OrderId, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置委员会抢单质押，单位： usd * 10^6, 如：16美元
        #[pallet::weight(0)]
        pub fn set_committee_order_stake(origin: OriginFor<T>, value: u64) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            CommitteeMinStake::<T>::put(value);
            Ok(().into())
        }

        // 设置报告人报告质押，单位：usd * 10^6, 如：16美元
        #[pallet::weight(0)]
        pub fn set_reporter_order_stake(origin: OriginFor<T>, value: u64) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            ReporterMinStake::<T>::put(value);
            Ok(().into())
        }

        // 需要Root权限。添加到委员会，直接添加到committee列表中
        #[pallet::weight(0)]
        pub fn add_committee(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut staker = Self::staker();

            staker.black_list.binary_search(&member).map_err(|_| Error::<T>::AccountAlreadyExist)?;
            staker.committee.binary_search(&member).map_err(|_| Error::<T>::AccountAlreadyExist)?;

            if let Ok(index) = staker.waiting_box_pubkey.binary_search(&member) {
                staker.committee.insert(index, member.clone());
            }

            Self::deposit_event(Event::CommitteeAdded(member));
            Ok(().into())
        }

        // 委员会需要手动添加自己的加密公钥信息
        #[pallet::weight(0)]
        pub fn committee_set_box_pubkey(origin: OriginFor<T>, box_pubkey: [u8; 32]) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let mut committee_list = Self::staker();

            // 不是委员会则返回错误
            if committee_list.committee.binary_search(&committee).is_err()
                && committee_list.waiting_box_pubkey.binary_search(&committee).is_err() {
                    return Err(Error::<T>::NotCommittee.into());
            }

            BoxPubkey::<T>::insert(&committee, box_pubkey);
            if let Ok(index) = committee_list.waiting_box_pubkey.binary_search(&committee) {
                committee_list.waiting_box_pubkey.remove(index);
            }

            Ok(().into())
        }

        // FIXME: 委员会列表中没有任何任务时，委员会可以退出
        #[pallet::weight(10000)]
        pub fn exit_staker(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut staker = Self::staker();
            staker.committee.binary_search(&who).map_err(|_|  Error::<T>::AccountNotExist)?;

            // 如果有未完成的工作，则不允许退出
            let committee_machines = Self::committee_machines(&who);
            if committee_machines.booked_order.len() > 0 ||
                committee_machines.hashed_order.len() > 0 ||
                committee_machines.confirmed_order.len() > 0 {
                return Err(Error::<T>::JobNotDone.into());
            }

            if let Ok(index) = staker.committee.binary_search(&who) {
                staker.committee.remove(index);
            }

            Staker::<T>::put(staker);

            Self::deposit_event(Event::ExitFromCandidacy(who));

            return Ok(().into());
        }

        // 任何用户可以报告机器有问题
        #[pallet::weight(10000)]
        pub fn report_machine_fault(origin: OriginFor<T>, raw_hash: Hash, box_public_key: [u8; 32]) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            let report_time = <frame_system::Module<T>>::block_number();

            let reporter_order_stake = Self::reporter_min_stake();
            let reporter_stake_need = Self::get_dbc_amount_by_value(reporter_order_stake).ok_or(Error::<T>::GetStakeAmountFailed)?;

            Self::add_user_total_stake(&reporter, reporter_stake_need).map_err(|_| Error::<T>::StakeFailed)?;

            // 被报告的机器存储起来，委员会进行抢单
            let mut live_order = Self::live_order();
            let order_id = Self::get_new_order_id();
            if let Err(index) = live_order.reported_order.binary_search(&order_id) {
                live_order.reported_order.insert(index, order_id);
            }
            LiveOrder::<T>::put(live_order);

            ReportedOrderInfo::<T>::insert(&order_id, MachineCommitteeList {
                order_id,
                report_time,
                raw_hash,
                box_public_key,
                reporter_stake: reporter_stake_need,
                machine_status: ReportStatus::Reported,
                ..Default::default()
            });

            // 记录到报告人的存储中
            let mut reporter_order = Self::reporter_order(&reporter);
            if let Err(index) = reporter_order.reported_id.binary_search(&order_id) {
                reporter_order.reported_id.insert(index, order_id);
            }
            ReporterOrder::<T>::insert(&reporter, reporter_order);

            Ok(().into())
        }

        // 报告人可以在抢单之前取消该报告
        #[pallet::weight(10000)]
        pub fn reporter_cancle_fault_report(origin: OriginFor<T>, order_id: OrderId) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;

            let order_detail = Self::reported_order_info(&order_id);
            ensure!(order_detail.machine_status == ReportStatus::Reported, Error::<T>::OrderNotAllowCancle);

            // 清理存储
            let mut live_order = Self::live_order();
            if let Ok(index) = live_order.reported_order.binary_search(&order_id) {
                live_order.reported_order.remove(index);
            }
            LiveOrder::<T>::put(live_order);

            let mut reporter_order = Self::reporter_order(&reporter);
            if let Ok(index) = reporter_order.reported_id.binary_search(&order_id) {
                reporter_order.reported_id.remove(index);
            }
            ReporterOrder::<T>::insert(&reporter, reporter_order);

            let order_info = Self::reported_order_info(&order_id);
            Self::reduce_user_total_stake(&reporter, order_info.reporter_stake);
            ReportedOrderInfo::<T>::remove(&order_id);

            Ok(().into())
        }

        // TODO: 增加该逻辑
        #[pallet::weight(10000)]
        pub fn report_machine_offline(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            todo!();
            Ok(().into())
        }

        // 委员会进行抢单
        #[pallet::weight(10000)]
        pub fn book_one(origin: OriginFor<T>, order_id: OrderId) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let committee_limit = Self::committee_limit();

            // 判断发起请求者是状态正常的委员会
            let staker = Self::staker();
            staker.committee.binary_search(&committee).map_err(|_| Error::<T>::NotCommittee)?;

            // 检查订单是否可预订状态
            let mut order_info = Self::reported_order_info(order_id);
            ensure!(order_info.machine_status == ReportStatus::WaitingBook, Error::<T>::OrderNotAllowBook);

            // 委员会增加质押
            let committee_order_stake = Self::committee_min_stake();
            let committee_stake_need = Self::get_dbc_amount_by_value(committee_order_stake).ok_or(Error::<T>::GetStakeAmountFailed)?;
            Self::add_user_total_stake(&committee, committee_stake_need).map_err(|_| Error::<T>::StakeFailed)?;

            // 记录第一个预订订单的时间
            if order_info.booked_committee.len() == 0 {
                order_info.first_book_time = now;
            }

            // 记录预订订单的委员会
            if let Err(index) = order_info.booked_committee.binary_search(&committee) {
                order_info.booked_committee.insert(index, committee.clone());
            } else {
                return Err(Error::<T>::AlreadyBooked.into());
            }

            // 达到要求的委员会人数，则变为验证中
            order_info.machine_status = ReportStatus::Verifying;

            // 如果达到委员会预订限制，则将该订单移动到fully_order列表
            let mut live_order = Self::live_order();
            if order_info.booked_committee.len() == committee_limit as usize {
                if let Ok(index) = live_order.reported_order.binary_search(&order_id) {
                    live_order.reported_order.remove(index);
                }
                if let Err(index) = live_order.fully_order.binary_search(&order_id) {
                    live_order.fully_order.insert(index, order_id);
                }
            }

            // 添加到委员会自己的存储中
            let mut committee_booked = Self::committee_machines(&committee);
            if let Err(index) = committee_booked.booked_order.binary_search(&order_id) {
                committee_booked.booked_order.insert(index, order_id);
            }
            CommitteeMachines::<T>::insert(&committee, committee_booked);

            // 添加委员会对于机器的操作记录
            let mut ops_detail = Self::committee_ops(&committee, &order_id);
            ops_detail.booked_time = now;
            CommitteeOps::<T>::insert(&committee, &order_id, ops_detail);

            ReportedOrderInfo::<T>::insert(&order_id, order_info);
            Ok(().into())
        }

        // 报告人在委员会完成抢单后，30分钟内用委员会的公钥，提交加密后的故障信息
        #[pallet::weight(10000)]
        pub fn reporter_add_encrypted_error_info(origin: OriginFor<T>, order_id: OrderId, to_committee: T::AccountId, encrypted_err_info: Vec<u8>, encrypted_info: Vec<u8>) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            // 检查该用户为order_id的reporter
            let reporter_order = Self::reporter_order(&to_committee);
            if let Err(_) = reporter_order.reported_id.binary_search(&order_id) {
                return Err(Error::<T>::NotOrderReporter.into());
            }

            // 且该orde处于验证中
            let mut reported_order_info = Self::reported_order_info(&order_id);
            ensure!(reported_order_info.machine_status == ReportStatus::Verifying, Error::<T>::OrderStatusNotFeat);

            // 检查该委员会为预订了该订单的委员会
            let reported_order_info = Self::reported_order_info(&order_id);
            if let Err(_) = reported_order_info.booked_committee.binary_search(&to_committee) {
                return Err(Error::<T>::NotOrderCommittee.into())
            }

            // 验证人提交了错误信息后，还能重新提交以修复，故不检查get_encrypted_info_committee

            // 添加到委员会对机器的信息中
            let mut committee_ops = Self::committee_ops(&to_committee, &order_id);

            committee_ops.encrypted_err_info = Some(encrypted_err_info);
            committee_ops.encrypted_time = now;
            CommitteeOps::<T>::insert(&to_committee, &order_id, committee_ops);

            // 检查是否为所有委员会提交了信息
            for a_committee in reported_order_info.booked_committee.iter() {
                let committee_ops = Self::committee_ops(&a_committee, &order_id);
                if let None = committee_ops.encrypted_err_info {
                    // 还有未提供加密信息的委员会
                    return Ok(().into())
                }
            }

            // 所有加密信息都已提供
            let mut live_order = Self::live_order();
            if let Ok(index) = live_order.fully_order.binary_search(&order_id) {
                live_order.fully_order.remove(index);
            }
            if let Err(index) = live_order.fully_report_encrypted_info.binary_search(&order_id) {
                live_order.fully_report_encrypted_info.insert(index, order_id)
            }
            LiveOrder::<T>::put(live_order);

            Ok(().into())
        }

        // 委员会提交验证之后的Hash
        #[pallet::weight(10000)]
        pub fn submit_confirm_hash(origin: OriginFor<T>, order_id: OrderId, hash: Hash) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let committee_limit = Self::committee_limit();

            // 判断是否为委员会其列表是否有该order_id
            let committee_booked = Self::committee_machines(&committee);
            committee_booked.booked_order.binary_search(&order_id).map_err(|_| Error::<T>::NotInBookedList)?;

            // 判断该order_id是否可以提交信息
            let mut reported_order_info = Self::reported_order_info(&order_id);

            ensure!(reported_order_info.machine_status == ReportStatus::Verifying
                    || reported_order_info.machine_status == ReportStatus::WaitingBook, Error::<T>::OrderStatusNotFeat);

            // 允许之后，添加到存储中
            let mut ops_detail = Self::committee_ops(&committee, &order_id);
            ops_detail.confirm_hash = hash;
            ops_detail.hash_time = now;
            CommitteeOps::<T>::insert(&committee, &order_id, ops_detail);

            if let Err(index) = reported_order_info.hashed_committee.binary_search(&committee) {
                reported_order_info.hashed_committee.insert(index, committee.clone());
            }

            // 判断是否已经全部提交了Hash，如果是，则改变该订单状态 // TODO: 移动到一个统一进程中
            if reported_order_info.hashed_committee.len() == committee_limit as usize {
                let mut committee_machine = Self::committee_machines(&committee);

                if let Ok(index) = committee_machine.booked_order.binary_search(&order_id) {
                    committee_machine.booked_order.remove(index);
                }

                if let Err(index) = committee_machine.hashed_order.binary_search(&order_id) {
                    committee_machine.hashed_order.insert(index, order_id);
                }

                CommitteeMachines::<T>::insert(&committee, committee_machine);

                let live_order = Self::live_order();
                if let Ok(index) = live_order.fully_report_encrypted_info.binary_search(&order_id) {
                    todo!()
                }
                if let Err(index) = live_order.fully_committee_hashed.binary_search(&order_id) {
                    todo!()
                }
            }

            ReportedOrderInfo::<T>::insert(&order_id, reported_order_info);
            Ok(().into())
        }

        #[pallet::weight(10000)]
        fn submit_confirm_raw(origin: OriginFor<T>, order_id: OrderId, machine_id: MachineId, reporter_rand_str: Vec<u8>, committee_rand_str: Vec<u8>, err_reason: Vec<u8>, support_report: bool) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut reported_order_info = Self::reported_order_info(order_id);
            ensure!(reported_order_info.machine_status == ReportStatus::SubmitingRaw, Error::<T>::OrderStatusNotFeat);

            // 检查是否提交了该订单的hash
            if let Err(_) = reported_order_info.hashed_committee.binary_search(&committee) {
                return Err(Error::<T>::NotProperCommittee.into());
            }

            let mut committee_ops = Self::committee_ops(&committee, &order_id);

            // 检查是否与报告人提交的Hash一致
            let mut reporter_info_raw = Vec::new();
            reporter_info_raw.extend(machine_id.clone());
            reporter_info_raw.extend(reporter_rand_str.clone());
            reporter_info_raw.extend(err_reason.clone());
            let reporter_report_hash = Self::get_hash(&reporter_info_raw);
            if reporter_report_hash != reported_order_info.raw_hash {
                return Err(Error::<T>::NotEqualReporterSubmit.into());
            }

            // 检查委员会提交是否与第一次Hash一致
            let mut committee_report_raw = Vec::new();
            committee_report_raw.extend(machine_id.clone());
            committee_report_raw.extend(reporter_rand_str);
            committee_report_raw.extend(committee_rand_str);
            let is_support: Vec<u8> = if support_report {
                "1".into()
            } else {
                "0".into()
            };
            committee_report_raw.extend(is_support);
            committee_report_raw.extend(err_reason.clone());
            let committee_report_hash = Self::get_hash(&committee_report_raw);
            if committee_report_hash != committee_ops.confirm_hash {
                return Err(Error::<T>::NotEqualCommitteeSubmit.into())
            }

            committee_ops.confirm_result = support_report;
            reported_order_info.err_info = err_reason;
            reported_order_info.machine_id = machine_id;

            CommitteeOps::<T>::insert(&committee, &order_id, committee_ops);
            ReportedOrderInfo::<T>::insert(&order_id, reported_order_info);

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
        NoBookableOrder,
        OrderStatusNotFeat,
        NotInBookedList,
        AlreadySubmitEncryptedInfo,
        NotOrderReporter,
        NotOrderCommittee,
        GetStakeAmountFailed,
        StakeFailed,
        OrderNotAllowCancle,
        OrderNotAllowBook,
        MaxCommitteeReached,
        NotProperCommittee,
        NotEqualReporterSubmit,
        NotEqualCommitteeSubmit,
    }
}

impl<T: Config> Pallet<T> {
    // 根据DBC价格获得最小质押数量
    // DBC精度15，Balance为u128, min_stake不超过10^24 usd 不会超出最大值
    fn get_dbc_amount_by_value(stake_value: u64) -> Option<BalanceOf<T>> {
        let one_dbc: BalanceOf<T> = 1000_000_000_000_000u64.saturated_into();
        let dbc_price = <dbc_price_ocw::Module<T>>::avg_price()?;

        one_dbc.checked_mul(&stake_value.saturated_into::<BalanceOf<T>>())?
            .checked_div(&dbc_price.saturated_into::<BalanceOf<T>>())
    }

    fn get_new_order_id() -> OrderId {
        let order_id = Self::next_order_id();
        NextOrderId::<T>::put(order_id + 1);
        return order_id;
    }

    fn get_hash(raw_str: &Vec<u8>) -> [u8; 16] {
        return blake2_128(raw_str);
    }

    fn add_user_total_stake(controller: &T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        let current_stake = Self::user_total_stake(controller);
        let next_stake = current_stake.checked_add(&amount).ok_or(())?;
        <T as pallet::Config>::Currency::set_lock(
            PALLET_LOCK_ID,
            controller,
            next_stake,
            WithdrawReasons::all(),
        );

        Ok(())
    }

    fn reduce_user_total_stake(controller: &T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        let current_stake = Self::user_total_stake(controller);
        let next_stake = current_stake.checked_sub(&amount).ok_or(())?;
        <T as pallet::Config>::Currency::set_lock(
            PALLET_LOCK_ID,
            controller,
            next_stake,
            WithdrawReasons::all(),
        );

        Ok(())
    }

    fn heart_beat() {
        let live_order = Self::live_order();
        // let now = Self::
        todo!()
    }
}
