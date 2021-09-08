#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{alloc::string::ToString, Decode, Encode};
use frame_support::{
    pallet_prelude::*,
    traits::{BalanceStatus, Currency, OnUnbalanced, ReservableCurrency},
    IterableStorageMap,
};
use frame_system::pallet_prelude::*;
use online_profile_machine::{MTOps, ManageCommittee};
use sp_io::hashing::blake2_128;
use sp_runtime::{
    traits::{CheckedSub, SaturatedConversion, Zero},
    Perbill, RuntimeDebug,
};
use sp_std::{collections::btree_set::BTreeSet, prelude::*, str, vec::Vec};

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

const HALF_HOUR: u32 = 60;
const ONE_HOUR: u32 = 120;
const THREE_HOUR: u32 = 360;
const FOUR_HOUR: u32 = 480;

pub type SlashId = u64;
pub type MachineId = Vec<u8>;
pub type ReportId = u64;
pub type BoxPubkey = [u8; 32];
pub type ReportHash = [u8; 16];
type BalanceOf<T> = <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

/// 机器故障的报告
/// 记录该模块中所有活跃的报告, 根据ReportStatus来区分
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTLiveReportList {
    /// 委员会可以抢单的报告
    pub bookable_report: Vec<ReportId>,
    /// 正在被验证的机器报告,验证完如能预定，转成上面状态，如不能则转成下面状态
    pub verifying_report: Vec<ReportId>,
    /// 等待提交原始值的报告, 所有委员会提交或时间截止，转为下面状态
    pub waiting_raw_report: Vec<ReportId>,
    /// 等待48小时后执行的报告, 此期间可以申述，由技术委员会审核
    pub finished_report: Vec<ReportId>,
}

impl MTLiveReportList {
    /// Add machine_id to one field of LiveMachine
    fn add_report_id(a_field: &mut Vec<ReportId>, report_id: ReportId) {
        if let Err(index) = a_field.binary_search(&report_id) {
            a_field.insert(index, report_id);
        }
    }

    /// Delete machine_id from one field of LiveMachine
    fn rm_report_id(a_field: &mut Vec<ReportId>, report_id: ReportId) {
        if let Ok(index) = a_field.binary_search(&report_id) {
            a_field.remove(index);
        }
    }
}

/// 报告人的报告记录
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct ReporterRecord {
    pub reported_id: Vec<ReportId>,
}

// 报告的详细信息
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTReportInfoDetail<AccountId, BlockNumber, Balance> {
    ///报告人
    pub reporter: AccountId,
    /// 报告提交时间
    pub report_time: BlockNumber,
    /// 报告人质押数量
    pub reporter_stake: Balance,
    /// 第一个委员会抢单时间
    pub first_book_time: BlockNumber,
    /// 出问题的机器，只有委员会提交原始信息时才存入
    pub machine_id: MachineId,
    /// 机器的故障原因
    pub err_info: Vec<u8>,
    /// 当前正在验证机器的委员会
    pub verifying_committee: Option<AccountId>,
    /// 抢单的委员会
    pub booked_committee: Vec<AccountId>,
    /// 获得报告人提交了加密信息的委员会列表
    pub get_encrypted_info_committee: Vec<AccountId>,
    /// 提交了检查报告Hash的委员会
    pub hashed_committee: Vec<AccountId>,
    /// 开始提交raw信息的时间
    pub confirm_start: BlockNumber,
    /// 提交了Raw信息的委员会
    pub confirmed_committee: Vec<AccountId>,
    /// 支持报告人的委员会
    pub support_committee: Vec<AccountId>,
    /// 不支持报告人的委员会
    pub against_committee: Vec<AccountId>,
    /// 当前报告的状态
    pub report_status: ReportStatus,
    /// 机器的故障类型
    pub machine_fault_type: MachineFaultType,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum ReportStatus {
    /// 没有委员会预订过的报告, 允许报告人取消
    Reported,
    /// 前一个委员会的报告已经超过一个小时，自动改成可预订状态
    WaitingBook,
    /// 有委员会抢单，处于验证中
    Verifying,
    /// 距离第一个验证人抢单3个小时后，等待委员会上传原始信息
    SubmittingRaw,
    /// 委员会已经完成，等待第48小时, 检查报告结果
    CommitteeConfirmed,
}

impl Default for ReportStatus {
    fn default() -> Self {
        ReportStatus::Reported
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MachineFaultType {
    /// 机器被租用，但无法访问的故障 (机器离线)
    RentedInaccessible(MachineId),
    /// 机器被租用，但有硬件故障
    RentedHardwareMalfunction(ReportHash, BoxPubkey),
    /// 机器被租用，但硬件参数造假
    RentedHardwareCounterfeit(ReportHash, BoxPubkey),
    /// 机器是在线状态，但无法租用
    OnlineRentFailed(ReportHash, BoxPubkey),
}

// 默认硬件故障
impl Default for MachineFaultType {
    fn default() -> Self {
        Self::RentedInaccessible(vec![])
    }
}

/// Summary after all committee submit raw info
enum ReportConfirmStatus<AccountId> {
    Confirmed(Vec<AccountId>, Vec<AccountId>, Vec<u8>),
    Refuse(Vec<AccountId>, Vec<AccountId>),
    NoConsensus,
}

/// 委员会抢到的报告的列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTCommitteeReportList {
    /// 委员会预订的报告
    pub booked_report: Vec<ReportId>,
    /// 已经提交了Hash信息的报告
    pub hashed_report: Vec<ReportId>,
    /// 已经提交了原始确认数据的报告
    pub confirmed_report: Vec<ReportId>,
    /// 已经成功上线的机器
    pub finished_report: Vec<MachineId>,
}

/// 委员会对报告的操作信息
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTCommitteeOpsDetail<BlockNumber, Balance> {
    pub booked_time: BlockNumber,
    /// reporter 提交的加密后的信息
    pub encrypted_err_info: Option<Vec<u8>>,
    pub encrypted_time: BlockNumber,
    pub confirm_hash: ReportHash,
    pub hash_time: BlockNumber,
    /// 委员会可以补充额外的信息
    pub extra_err_info: Vec<u8>,
    /// 委员会提交raw信息的时间
    pub confirm_time: BlockNumber,
    pub confirm_result: bool,
    pub staked_balance: Balance,
    pub order_status: MTOrderStatus,
}

/// 委员会抢单之后，对应订单的状态
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MTOrderStatus {
    /// 预订报告，状态将等待加密信息
    WaitingEncrypt,
    /// 获得加密信息之后，状态将等待加密信息
    Verifying,
    /// 等待提交原始信息
    WaitingRaw,
    /// 委员会已经完成了全部操作
    Finished,
}

impl Default for MTOrderStatus {
    fn default() -> Self {
        MTOrderStatus::Verifying
    }
}

/// Reporter stake params
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct ReporterStakeParamsInfo<Balance> {
    /// First time when report
    pub stake_baseline: Balance,
    /// How much stake will be used each report
    pub stake_per_report: Balance,
    /// 当剩余的质押数量到阈值时，需要补质押
    pub min_free_stake_percent: Perbill,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct ReporterStakeInfo<Balance> {
    pub staked_amount: Balance,
    pub used_stake: Balance,
    pub can_claim_reward: Balance,
    pub claimed_reward: Balance,
}

// 即将被执行的罚款
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTPendingSlashInfo<AccountId, BlockNumber, Balance> {
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
    /// 报告人被惩罚的原因
    pub slash_reason: MTReporterSlashReason,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MTReporterSlashReason {
    ReportRefused,
    NotSubmitEncryptedInfo,
}

impl Default for MTReporterSlashReason {
    fn default() -> Self {
        Self::ReportRefused
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + generic_func::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type ManageCommittee: ManageCommittee<
            AccountId = Self::AccountId,
            BalanceOf = BalanceOf<Self>,
            SlashReason = committee::CMSlashReason,
        >;
        type MTOps: MTOps<
            AccountId = Self::AccountId,
            MachineId = MachineId,
            FaultType = online_profile::OPSlashReason<Self::BlockNumber>,
        >;
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_n: BlockNumberFor<T>) -> frame_support::weights::Weight {
            let _ = Self::check_and_exec_slash();
            0
        }

        fn on_finalize(_block_number: T::BlockNumber) {
            let _ = Self::summary_fault_case();
            let _ = Self::summary_offline_case();
        }
    }

    #[pallet::type_value]
    pub fn CommitteeLimitDefault<T: Config>() -> u32 {
        3
    }

    /// Number of available committees for maintain module
    #[pallet::storage]
    #[pallet::getter(fn committee_limit)]
    pub(super) type CommitteeLimit<T: Config> = StorageValue<_, u32, ValueQuery, CommitteeLimitDefault<T>>;

    /// Report record for reporter
    #[pallet::storage]
    #[pallet::getter(fn reporter_report)]
    pub(super) type ReporterReport<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, ReporterRecord, ValueQuery>;

    // 通过报告单据ID，查询报告的机器的信息(委员会抢单信息)
    #[pallet::storage]
    #[pallet::getter(fn report_info)]
    pub(super) type ReportInfo<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        ReportId,
        MTReportInfoDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn reporter_stake_params)]
    pub(super) type ReporterStakeParams<T: Config> = StorageValue<_, ReporterStakeParamsInfo<BalanceOf<T>>>;

    #[pallet::storage]
    #[pallet::getter(fn reporter_stake)]
    pub(super) type ReporterStake<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, ReporterStakeInfo<BalanceOf<T>>, ValueQuery>;

    // 委员会查询自己的抢单信息
    #[pallet::storage]
    #[pallet::getter(fn committee_order)]
    pub(super) type CommitteeOrder<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, MTCommitteeReportList, ValueQuery>;

    // 存储委员会对单台机器的操作记录
    #[pallet::storage]
    #[pallet::getter(fn committee_ops)]
    pub(super) type CommitteeOps<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        ReportId,
        MTCommitteeOpsDetail<T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn next_report_id)]
    pub(super) type NextReportId<T: Config> = StorageValue<_, ReportId, ValueQuery>;

    /// 系统中还未完成的订单
    #[pallet::storage]
    #[pallet::getter(fn live_report)]
    pub(super) type LiveReport<T: Config> = StorageValue<_, MTLiveReportList, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_slash_id)]
    pub(super) type NextSlashId<T: Config> = StorageValue<_, SlashId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pending_slash)]
    pub(super) type PendingSlash<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        SlashId,
        MTPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(0)]
        pub fn set_reporter_stake_params(
            origin: OriginFor<T>,
            stake_params: ReporterStakeParamsInfo<BalanceOf<T>>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            ReporterStakeParams::<T>::put(stake_params);
            Ok(().into())
        }

        /// 用户报告机器有故障：无法租用或者硬件故障或者离线
        /// 报告无法租用提交Hash:机器ID+随机数+报告内容
        /// 报告硬件故障提交Hash:机器ID+随机数+报告内容+租用机器的Session信息
        /// 用户报告机器硬件故障
        #[pallet::weight(10000)]
        pub fn report_machine_fault(
            origin: OriginFor<T>,
            report_reason: MachineFaultType,
        ) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            Self::report_handler(reporter, report_reason)
        }

        #[pallet::weight(10000)]
        pub fn reporter_add_stake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            let stake_params = Self::reporter_stake_params().ok_or(Error::<T>::GetStakeAmountFailed)?;
            let mut reporter_stake = Self::reporter_stake(&reporter);

            reporter_stake.staked_amount += amount;
            ensure!(
                reporter_stake.staked_amount - reporter_stake.used_stake >
                    stake_params.min_free_stake_percent * reporter_stake.staked_amount,
                Error::<T>::StakeNotEnough
            );
            ensure!(<T as Config>::Currency::can_reserve(&reporter, amount), Error::<T>::BalanceNotEnough);

            <T as pallet::Config>::Currency::reserve(&reporter, amount).map_err(|_| Error::<T>::BalanceNotEnough)?;

            ReporterStake::<T>::insert(&reporter, reporter_stake);
            Ok(().into())
        }

        // 报告人可以在抢单之前取消该报告
        #[pallet::weight(10000)]
        pub fn reporter_cancel_report(origin: OriginFor<T>, report_id: ReportId) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;

            let report_info = Self::report_info(&report_id);
            ensure!(report_info.report_status == ReportStatus::Reported, Error::<T>::OrderNotAllowCancel);

            // 清理存储
            let mut live_report = Self::live_report();
            if let Ok(index) = live_report.bookable_report.binary_search(&report_id) {
                live_report.bookable_report.remove(index);
            }

            let mut reporter_report = Self::reporter_report(&reporter);
            if let Ok(index) = reporter_report.reported_id.binary_search(&report_id) {
                reporter_report.reported_id.remove(index);
            }

            let mut reporter_stake = Self::reporter_stake(&reporter);
            reporter_stake.used_stake = reporter_stake
                .used_stake
                .checked_sub(&report_info.reporter_stake)
                .ok_or(Error::<T>::ReduceTotalStakeFailed)?;

            let _ = <T as pallet::Config>::Currency::unreserve(&reporter, report_info.reporter_stake);

            ReporterStake::<T>::insert(&reporter, reporter_stake);
            ReporterReport::<T>::insert(&reporter, reporter_report);
            LiveReport::<T>::put(live_report);
            ReportInfo::<T>::remove(&report_id);

            Self::deposit_event(Event::ReportCanceld(reporter, report_id, report_info.machine_fault_type));
            Ok(().into())
        }

        /// 委员会进行抢单
        #[pallet::weight(10000)]
        pub fn book_fault_order(origin: OriginFor<T>, report_id: ReportId) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            // 判断发起请求者是状态正常的委员会
            ensure!(T::ManageCommittee::is_valid_committee(&committee), Error::<T>::NotCommittee);
            ensure!(<ReportInfo<T>>::contains_key(report_id), Error::<T>::OrderNotAllowBook);

            // 检查订单是否可预订状态
            let mut live_report = Self::live_report();
            let mut report_info = Self::report_info(report_id);
            let mut ops_detail = Self::committee_ops(&committee, &report_id);
            let mut is_live_report_changed = false;

            // 检查订单是否可以抢定
            ensure!(
                report_info.report_status == ReportStatus::Reported ||
                    report_info.report_status == ReportStatus::WaitingBook ||
                    report_info.booked_committee.len() < 3,
                Error::<T>::OrderNotAllowBook
            );

            // 记录预订订单的委员会
            if let Err(index) = report_info.booked_committee.binary_search(&committee) {
                report_info.booked_committee.insert(index, committee.clone());
                // 记录第一个预订订单的时间, 3个小时(360个块)之后开始提交原始值
                if report_info.booked_committee.len() == 1 {
                    report_info.first_book_time = now;
                    report_info.confirm_start = now + 360u32.saturated_into::<T::BlockNumber>();
                }
            } else {
                return Err(Error::<T>::AlreadyBooked.into())
            }

            // 添加委员会对于机器的操作记录
            ops_detail.booked_time = now;

            // 支付手续费或押金
            match report_info.machine_fault_type {
                MachineFaultType::RentedInaccessible(..) => {
                    // 付10个DBC的手续费
                    <generic_func::Module<T>>::pay_fixed_tx_fee(committee.clone())
                        .map_err(|_| Error::<T>::PayTxFeeFailed)?;

                    ops_detail.order_status = MTOrderStatus::Verifying;
                    // WaitingBook状态允许其他委员会继续抢单
                    report_info.report_status = if report_info.booked_committee.len() == 3 {
                        ReportStatus::Verifying
                    } else {
                        ReportStatus::WaitingBook
                    }
                },
                // 其他情况，需要质押100RMB等值DBC
                MachineFaultType::RentedHardwareMalfunction(..) |
                MachineFaultType::RentedHardwareCounterfeit(..) |
                MachineFaultType::OnlineRentFailed(..) => {
                    // 支付质押
                    let committee_order_stake =
                        T::ManageCommittee::stake_per_order().ok_or(Error::<T>::GetStakeAmountFailed)?;
                    <T as pallet::Config>::ManageCommittee::change_used_stake(
                        committee.clone(),
                        committee_order_stake,
                        true,
                    )
                    .map_err(|_| Error::<T>::StakeFailed)?;
                    ops_detail.staked_balance = committee_order_stake;
                    ops_detail.order_status = MTOrderStatus::WaitingEncrypt;

                    // 改变report状态为正在验证中，此时禁止其他委员会预订
                    report_info.report_status = ReportStatus::Verifying;

                    // 从bookable_report移动到verifying_report
                    if let Ok(index) = live_report.bookable_report.binary_search(&report_id) {
                        live_report.bookable_report.remove(index);
                    }
                    if let Err(index) = live_report.verifying_report.binary_search(&report_id) {
                        live_report.verifying_report.insert(index, report_id);
                    }
                    is_live_report_changed = true;
                },
            }

            // 记录当前哪个委员会正在验证，方便状态控制
            report_info.verifying_committee = Some(committee.clone());

            // 添加到委员会自己的存储中
            let mut committee_order = Self::committee_order(&committee);
            if let Err(index) = committee_order.booked_report.binary_search(&report_id) {
                committee_order.booked_report.insert(index, report_id);
            }

            if is_live_report_changed {
                LiveReport::<T>::put(live_report);
            }
            CommitteeOps::<T>::insert(&committee, &report_id, ops_detail);
            CommitteeOrder::<T>::insert(&committee, committee_order);
            ReportInfo::<T>::insert(&report_id, report_info);

            Ok(().into())
        }

        /// 报告人在委员会完成抢单后，30分钟内用委员会的公钥，提交加密后的故障信息
        /// 只有报告机器故障或者无法租用时需要提交加密信息
        #[pallet::weight(10000)]
        pub fn reporter_add_encrypted_error_info(
            origin: OriginFor<T>,
            report_id: ReportId,
            to_committee: T::AccountId,
            encrypted_err_info: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            // 该orde处于验证中, 且还没有提交过加密信息
            let mut report_info = Self::report_info(&report_id);
            ensure!(&report_info.reporter == &reporter, Error::<T>::NotOrderReporter);
            ensure!(report_info.report_status == ReportStatus::Verifying, Error::<T>::OrderStatusNotFeat);
            if let MachineFaultType::RentedInaccessible(..) = report_info.machine_fault_type {
                return Err(Error::<T>::NotNeedEncryptedInfo.into())
            }

            let mut committee_ops = Self::committee_ops(&to_committee, &report_id);
            ensure!(committee_ops.order_status == MTOrderStatus::WaitingEncrypt, Error::<T>::OrderStatusNotFeat);
            // 检查该委员会为预订了该订单的委员会
            if report_info.booked_committee.binary_search(&to_committee).is_err() {
                return Err(Error::<T>::NotOrderCommittee.into())
            }

            // report_info中插入已经收到了加密信息的委员会
            if let Err(index) = report_info.get_encrypted_info_committee.binary_search(&to_committee) {
                report_info.get_encrypted_info_committee.insert(index, to_committee.clone());
            }

            committee_ops.encrypted_err_info = Some(encrypted_err_info);
            committee_ops.encrypted_time = now;
            committee_ops.order_status = MTOrderStatus::Verifying;

            CommitteeOps::<T>::insert(&to_committee, &report_id, committee_ops);
            ReportInfo::<T>::insert(report_id, report_info);

            Self::deposit_event(Event::EncryptedInfoSent(reporter, to_committee, report_id));
            Ok(().into())
        }

        // 委员会提交验证之后的Hash
        // 用户必须在自己的Order状态为Verifying时提交Hash
        #[pallet::weight(10000)]
        pub fn submit_confirm_hash(
            origin: OriginFor<T>,
            report_id: ReportId,
            hash: ReportHash,
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let committee_limit = Self::committee_limit();

            // 判断是否为委员会其列表是否有该report_id
            let mut committee_order = Self::committee_order(&committee);
            let mut committee_ops = Self::committee_ops(&committee, &report_id);
            let mut report_info = Self::report_info(&report_id);
            let mut live_report = Self::live_report();

            if committee_order.booked_report.binary_search(&report_id).is_err() {
                return Err(Error::<T>::NotInBookedList.into())
            }
            // 判断该委员会的状态是验证中
            ensure!(committee_ops.order_status == MTOrderStatus::Verifying, Error::<T>::OrderStatusNotFeat);
            // 判断该report_id是否可以提交信息
            if let MachineFaultType::RentedInaccessible(..) = report_info.machine_fault_type {
                ensure!(
                    report_info.report_status == ReportStatus::WaitingBook ||
                        report_info.report_status == ReportStatus::Verifying,
                    Error::<T>::OrderStatusNotFeat
                );
            } else {
                ensure!(report_info.report_status == ReportStatus::Verifying, Error::<T>::OrderStatusNotFeat);
            }

            // 添加到report的已提交Hash的委员会列表
            if let Err(index) = report_info.hashed_committee.binary_search(&committee) {
                report_info.hashed_committee.insert(index, committee.clone());
            }

            // 判断是否已经有3个了
            if report_info.hashed_committee.len() == committee_limit as usize {
                // 满足要求的Hash已镜提交，则进入提交raw的阶段
                if let Ok(index) = live_report.verifying_report.binary_search(&report_id) {
                    live_report.verifying_report.remove(index);
                }
                if let Err(index) = live_report.waiting_raw_report.binary_search(&report_id) {
                    live_report.waiting_raw_report.insert(index, report_id);
                }

                report_info.report_status = ReportStatus::SubmittingRaw;
            } else {
                if let Ok(index) = live_report.verifying_report.binary_search(&report_id) {
                    live_report.verifying_report.remove(index);
                }
                if let Err(index) = live_report.bookable_report.binary_search(&report_id) {
                    live_report.bookable_report.insert(index, report_id);
                }
                report_info.report_status = ReportStatus::WaitingBook;
            }

            report_info.verifying_committee = None;

            // 修改committeeOps存储/状态
            committee_ops.order_status = MTOrderStatus::WaitingRaw;
            committee_ops.confirm_hash = hash;
            committee_ops.hash_time = now;

            // 将订单从委员会已预订移动到已Hash
            if let Ok(index) = committee_order.booked_report.binary_search(&report_id) {
                committee_order.booked_report.remove(index);
            }
            if let Err(index) = committee_order.hashed_report.binary_search(&report_id) {
                committee_order.hashed_report.insert(index, report_id);
            }

            LiveReport::<T>::put(live_report);
            CommitteeOps::<T>::insert(&committee, &report_id, committee_ops);
            CommitteeOrder::<T>::insert(&committee, committee_order);
            ReportInfo::<T>::insert(&report_id, report_info);

            Self::deposit_event(Event::HashSubmited(report_id, committee));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn submit_offline_raw(
            origin: OriginFor<T>,
            report_id: ReportId,
            committee_rand_str: Vec<u8>,
            is_support: bool,
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut report_info = Self::report_info(report_id);
            ensure!(report_info.report_status == ReportStatus::SubmittingRaw, Error::<T>::OrderStatusNotFeat);
            match report_info.machine_fault_type {
                MachineFaultType::RentedInaccessible(..) => {},
                _ => return Err(Error::<T>::OrderStatusNotFeat.into()),
            }

            // 检查是否提交了该订单的hash
            report_info.hashed_committee.binary_search(&committee).map_err(|_| Error::<T>::NotProperCommittee)?;

            // 添加到Report的已提交Raw的列表
            if let Err(index) = report_info.confirmed_committee.binary_search(&committee) {
                report_info.confirmed_committee.insert(index, committee.clone());
            }

            let mut committee_ops = Self::committee_ops(&committee, &report_id);
            // 计算Hash
            let mut raw_msg_info = Vec::new();
            let new_report_id: Vec<u8> = report_id.to_string().into();
            raw_msg_info.extend(new_report_id);
            raw_msg_info.extend(committee_rand_str);
            let is_support_u8: Vec<u8> = if is_support { "1".into() } else { "0".into() };
            raw_msg_info.extend(is_support_u8);
            ensure!(Self::get_hash(&raw_msg_info) == committee_ops.confirm_hash, Error::<T>::NotEqualCommitteeSubmit);

            // 将委员会插入到是否支持的委员会列表
            if is_support {
                if let Err(index) = report_info.support_committee.binary_search(&committee) {
                    report_info.support_committee.insert(index, committee.clone())
                }
            } else {
                if let Err(index) = report_info.support_committee.binary_search(&committee) {
                    report_info.against_committee.insert(index, committee.clone())
                }
            }

            committee_ops.confirm_time = now;
            committee_ops.confirm_result = is_support;

            CommitteeOps::<T>::insert(&committee, &report_id, committee_ops);
            ReportInfo::<T>::insert(&report_id, report_info);

            Self::deposit_event(Event::RawInfoSubmited(report_id, committee));
            Ok(().into())
        }

        /// 订单状态必须是等待SubmittingRaw: 除了offline之外的所有错误类型
        #[pallet::weight(10000)]
        pub fn submit_confirm_raw(
            origin: OriginFor<T>,
            report_id: ReportId,
            machine_id: MachineId,
            reporter_rand_str: Vec<u8>,
            committee_rand_str: Vec<u8>,
            err_reason: Vec<u8>,
            extra_err_info: Vec<u8>,
            support_report: bool,
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut report_info = Self::report_info(report_id);
            ensure!(report_info.report_status == ReportStatus::SubmittingRaw, Error::<T>::OrderStatusNotFeat);

            if let MachineFaultType::OnlineRentFailed(..) = report_info.machine_fault_type {
                return Err(Error::<T>::OrderStatusNotFeat.into())
            }

            let reporter_hash = match report_info.machine_fault_type {
                MachineFaultType::RentedInaccessible(..) => return Err(Error::<T>::OrderStatusNotFeat.into()),
                MachineFaultType::RentedHardwareMalfunction(hash, _) => hash,
                MachineFaultType::RentedHardwareCounterfeit(hash, _) => hash,
                MachineFaultType::OnlineRentFailed(hash, _) => hash,
            };

            // 检查是否提交了该订单的hash
            report_info.hashed_committee.binary_search(&committee).map_err(|_| Error::<T>::NotProperCommittee)?;

            // 添加到Report的已提交Raw的列表
            if let Err(index) = report_info.confirmed_committee.binary_search(&committee) {
                report_info.confirmed_committee.insert(index, committee.clone());
            }

            let mut committee_ops = Self::committee_ops(&committee, &report_id);

            // 检查是否与报告人提交的Hash一致
            let mut reporter_info_raw = Vec::new();
            reporter_info_raw.extend(machine_id.clone());
            reporter_info_raw.extend(reporter_rand_str.clone());
            reporter_info_raw.extend(err_reason.clone());
            let reporter_report_hash = Self::get_hash(&reporter_info_raw);
            ensure!(reporter_report_hash == reporter_hash, Error::<T>::NotEqualReporterSubmit);

            // 检查委员会提交是否与第一次Hash一致
            let mut committee_report_raw = Vec::new();
            committee_report_raw.extend(machine_id.clone());
            committee_report_raw.extend(reporter_rand_str);
            committee_report_raw.extend(committee_rand_str);
            let is_support: Vec<u8> = if support_report { "1".into() } else { "0".into() };
            committee_report_raw.extend(is_support);
            committee_report_raw.extend(err_reason.clone());
            committee_report_raw.extend(extra_err_info.clone());
            let committee_report_hash = Self::get_hash(&committee_report_raw);
            ensure!(committee_report_hash == committee_ops.confirm_hash, Error::<T>::NotEqualCommitteeSubmit);

            // 将委员会插入到是否支持的委员会列表
            if support_report {
                if let Err(index) = report_info.support_committee.binary_search(&committee) {
                    report_info.support_committee.insert(index, committee.clone())
                }
            } else {
                if let Err(index) = report_info.support_committee.binary_search(&committee) {
                    report_info.against_committee.insert(index, committee.clone())
                }
            }

            report_info.machine_id = machine_id;
            report_info.err_info = err_reason;
            committee_ops.confirm_time = now;
            committee_ops.confirm_result = support_report;
            committee_ops.extra_err_info = extra_err_info;
            committee_ops.order_status = MTOrderStatus::Finished;

            CommitteeOps::<T>::insert(&committee, &report_id, committee_ops);
            ReportInfo::<T>::insert(&report_id, report_info);

            Self::deposit_event(Event::RawInfoSubmited(report_id, committee));
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ReportMachineFault(T::AccountId, MachineFaultType),
        ReportCanceld(T::AccountId, ReportId, MachineFaultType),
        EncryptedInfoSent(T::AccountId, T::AccountId, ReportId),
        HashSubmited(ReportId, T::AccountId),
        RawInfoSubmited(ReportId, T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        NotCommittee,
        AlreadyBooked,
        OrderStatusNotFeat,
        NotInBookedList,
        NotOrderReporter,
        NotOrderCommittee,
        GetStakeAmountFailed,
        StakeFailed,
        OrderNotAllowCancel,
        OrderNotAllowBook,
        NotProperCommittee,
        NotEqualReporterSubmit,
        NotEqualCommitteeSubmit,
        ReduceTotalStakeFailed,
        PayTxFeeFailed,
        NotNeedEncryptedInfo,
        ExpiredReport,
        AlreadySubmitConfirmation,
        BalanceNotEnough,
        StakeNotEnough,
        BoxPubkeyIsNoneInFirstReport,
    }
}

impl<T: Config> Pallet<T> {
    fn check_and_exec_slash() -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let pending_slash_id = Self::get_all_slash_id();

        for slash_id in pending_slash_id {
            let slash_info = Self::pending_slash(&slash_id);
            if now >= slash_info.slash_exec_time {
                // 如果reward_to为0，则将币转到国库
                let reward_to_num = slash_info.reward_to.len() as u32;

                let mut reporter_stake = Self::reporter_stake(&slash_info.slash_who);

                // let mut committee_stake = Self::committee_stake(&slash_info.slash_who);

                reporter_stake.used_stake = reporter_stake.used_stake.checked_sub(&slash_info.slash_amount).ok_or(())?;
                reporter_stake.staked_amount =
                    reporter_stake.staked_amount.checked_sub(&slash_info.slash_amount).ok_or(())?;

                if reward_to_num == 0 {
                    if <T as pallet::Config>::Currency::can_slash(&slash_info.slash_who, slash_info.slash_amount) {
                        let (imbalance, _missing) =
                            <T as pallet::Config>::Currency::slash(&slash_info.slash_who, slash_info.slash_amount);
                        <T as pallet::Config>::Slash::on_unbalanced(imbalance);

                        PendingSlash::<T>::remove(slash_id);
                    }
                } else {
                    let reward_each_get =
                        Perbill::from_rational_approximation(1u32, reward_to_num) * slash_info.slash_amount;
                    let mut left_reward = slash_info.slash_amount;

                    for a_committee in slash_info.reward_to {
                        if <T as pallet::Config>::Currency::can_slash(&slash_info.slash_who, left_reward) {
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

                ReporterStake::<T>::insert(&slash_info.slash_who, reporter_stake);
                PendingSlash::<T>::remove(slash_id);
            }
        }
        Ok(())
    }

    // 获得所有被惩罚的订单列表
    fn get_all_slash_id() -> BTreeSet<SlashId> {
        <PendingSlash<T> as IterableStorageMap<SlashId, _>>::iter()
            .map(|(slash_id, _)| slash_id)
            .collect::<BTreeSet<_>>()
    }

    pub fn report_handler(reporter: T::AccountId, machine_fault_type: MachineFaultType) -> DispatchResultWithPostInfo {
        let now = <frame_system::Module<T>>::block_number();
        let report_id = Self::get_new_report_id();
        let stake_params = Self::reporter_stake_params().ok_or(Error::<T>::GetStakeAmountFailed)?;

        let mut reporter_stake = Self::reporter_stake(&reporter);
        let mut report_info = MTReportInfoDetail {
            reporter: reporter.clone(),
            report_time: now,
            reporter_stake: stake_params.stake_per_report,
            machine_fault_type: machine_fault_type.clone(),
            report_status: ReportStatus::Reported,
            ..Default::default()
        };

        if let MachineFaultType::RentedInaccessible(machine_id) = machine_fault_type.clone() {
            <generic_func::Module<T>>::pay_fixed_tx_fee(reporter.clone()).map_err(|_| Error::<T>::PayTxFeeFailed)?;
            report_info.machine_id = machine_id;
        }

        // 3种报告类型，都需要质押 1000 DBC
        // 如果是第一次绑定，则需要质押2w DBC，其他情况:
        if reporter_stake.staked_amount == Zero::zero() {
            if !<T as Config>::Currency::can_reserve(&reporter, stake_params.stake_baseline) {
                return Err(Error::<T>::BalanceNotEnough.into())
            }

            <T as pallet::Config>::Currency::reserve(&reporter, stake_params.stake_baseline)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;
            reporter_stake.staked_amount = stake_params.stake_baseline;
            reporter_stake.used_stake = stake_params.stake_per_report;
        } else {
            reporter_stake.used_stake += stake_params.stake_per_report;
            if reporter_stake.staked_amount - reporter_stake.used_stake >
                stake_params.min_free_stake_percent * reporter_stake.staked_amount
            {
                return Err(Error::<T>::StakeNotEnough.into())
            }
        }

        let mut live_report = Self::live_report();
        if let Err(index) = live_report.bookable_report.binary_search(&report_id) {
            live_report.bookable_report.insert(index, report_id);
        }

        // 记录到报告人的存储中
        let mut reporter_report = Self::reporter_report(&reporter);
        if let Err(index) = reporter_report.reported_id.binary_search(&report_id) {
            reporter_report.reported_id.insert(index, report_id);
        }

        ReporterStake::<T>::insert(&reporter, reporter_stake);
        ReportInfo::<T>::insert(&report_id, report_info);
        LiveReport::<T>::put(live_report);
        ReporterReport::<T>::insert(&reporter, reporter_report);
        Self::deposit_event(Event::ReportMachineFault(reporter, machine_fault_type));

        Ok(().into())
    }

    fn get_new_report_id() -> ReportId {
        let report_id = Self::next_report_id();

        if report_id == u64::MAX {
            NextReportId::<T>::put(0);
        } else {
            NextReportId::<T>::put(report_id + 1);
        };

        return report_id
    }

    fn get_new_slash_id() -> SlashId {
        let slash_id = Self::next_slash_id();

        if slash_id == u64::MAX {
            NextSlashId::<T>::put(0);
        } else {
            NextSlashId::<T>::put(slash_id + 1);
        };

        return slash_id
    }

    fn add_slash(
        who: T::AccountId,
        amount: BalanceOf<T>,
        reward_to: Vec<T::AccountId>,
        slash_reason: MTReporterSlashReason,
    ) {
        let slash_id = Self::get_new_slash_id();
        let now = <frame_system::Module<T>>::block_number();
        PendingSlash::<T>::insert(
            slash_id,
            MTPendingSlashInfo {
                slash_who: who,
                slash_time: now,
                slash_amount: amount,
                slash_exec_time: now + 5760u32.saturated_into::<T::BlockNumber>(),
                reward_to,
                slash_reason,
            },
        );
    }

    fn get_hash(raw_str: &Vec<u8>) -> [u8; 16] {
        return blake2_128(raw_str)
    }

    // 处理用户没有发送加密信息的订单
    // 对用户进行惩罚，对委员会进行奖励
    fn refund_committee_clean_report(report_id: ReportId) {
        let report_info = Self::report_info(report_id);

        // 清理每个委员会存储
        for a_committee in report_info.booked_committee {
            let committee_ops = Self::committee_ops(&a_committee, &report_id);

            let _ = <T as pallet::Config>::ManageCommittee::change_used_stake(
                a_committee.clone(),
                committee_ops.staked_balance,
                false,
            );

            CommitteeOps::<T>::remove(&a_committee, &report_id);

            Self::clean_from_committee_order(&a_committee, &report_id);
        }

        // 清理该报告
        Self::clean_from_live_report(&report_id);
        ReportInfo::<T>::remove(&report_id);
    }

    // rm from committee_order
    fn clean_from_committee_order(committee: &T::AccountId, report_id: &ReportId) {
        let mut committee_order = Self::committee_order(committee);
        if let Ok(index) = committee_order.booked_report.binary_search(report_id) {
            committee_order.booked_report.remove(index);
        }
        if let Ok(index) = committee_order.hashed_report.binary_search(report_id) {
            committee_order.hashed_report.remove(index);
        }
        if let Ok(index) = committee_order.confirmed_report.binary_search(report_id) {
            committee_order.confirmed_report.remove(index);
        }

        CommitteeOrder::<T>::insert(committee, committee_order);
    }

    // rm from live_report
    fn clean_from_live_report(report_id: &ReportId) {
        let mut live_report = Self::live_report();
        if let Ok(index) = live_report.bookable_report.binary_search(report_id) {
            live_report.bookable_report.remove(index);
        }
        if let Ok(index) = live_report.verifying_report.binary_search(report_id) {
            live_report.verifying_report.remove(index);
        }
        if let Ok(index) = live_report.waiting_raw_report.binary_search(report_id) {
            live_report.waiting_raw_report.remove(index);
        }
        if let Ok(index) = live_report.finished_report.binary_search(report_id) {
            live_report.finished_report.remove(index);
        }
        LiveReport::<T>::put(live_report);
    }

    // - Writes:
    // CommitteeOps, MTCommitteeReportList, MTLiveReportList
    // NOTE: MTReportInfoDetail 不清理
    fn clean_finished_order(report_id: ReportId) {
        let report_info = Self::report_info(report_id);
        for a_committee in report_info.booked_committee {
            CommitteeOps::<T>::remove(&a_committee, report_id);
            Self::clean_from_committee_order(&a_committee, &report_id);
            Self::clean_from_live_report(&report_id);
        }
    }

    // Summary committee's handle result
    fn summary_report(report_id: ReportId) -> ReportConfirmStatus<T::AccountId> {
        let report_info = Self::report_info(&report_id);
        // 如果没有委员会提交Raw信息，则无共识
        if report_info.confirmed_committee.len() == 0 {
            return ReportConfirmStatus::NoConsensus
        }

        if report_info.support_committee.len() >= report_info.against_committee.len() {
            return ReportConfirmStatus::Confirmed(
                report_info.support_committee,
                report_info.against_committee,
                report_info.err_info.clone(),
            )
        }

        return ReportConfirmStatus::Refuse(report_info.support_committee, report_info.against_committee)
    }

    // 惩罚掉线的机器
    fn summary_offline_case() -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let live_report = Self::live_report();

        for report_id in live_report.bookable_report.clone() {
            let mut report_info = Self::report_info(&report_id);
            match report_info.machine_fault_type {
                // 仅处理Offline的情况
                MachineFaultType::RentedInaccessible(..) => {},
                _ => continue,
            }

            // 当大于等于10分钟，或者提交确认的委员会等于提交了hash的委员会，需要执行后面的逻辑，来确认
            if now - report_info.first_book_time >= 20u32.into() ||
                report_info.confirmed_committee.len() == report_info.hashed_committee.len()
            {
                // 统计预订了但没有提交确认的委员会
                for a_committee in report_info.booked_committee {
                    if report_info.confirmed_committee.binary_search(&a_committee).is_err() {
                        let committee_ops = Self::committee_ops(&a_committee, report_id);
                        <T as pallet::Config>::ManageCommittee::add_slash(
                            a_committee.clone(),
                            committee_ops.staked_balance,
                            Vec::new(),
                            committee::CMSlashReason::MCNotSubmitRaw,
                        );
                    }
                }

                if report_info.support_committee >= report_info.against_committee {
                    // 此时，应该支持报告人，惩罚反对的委员会
                    T::MTOps::mt_machine_offline(
                        report_info.reporter.clone(),
                        report_info.support_committee,
                        report_info.machine_id.clone(),
                        online_profile::OPSlashReason::RentedInaccessible(report_info.report_time),
                    );
                    for a_committee in report_info.against_committee {
                        let committee_ops = Self::committee_ops(&a_committee, report_id);
                        <T as pallet::Config>::ManageCommittee::add_slash(
                            a_committee.clone(),
                            committee_ops.staked_balance,
                            Vec::new(),
                            committee::CMSlashReason::MCInconsistentSubmit,
                        );
                    }
                } else {
                    // 此时，应该否决报告人，处理委员会
                    Self::add_slash(
                        report_info.reporter,
                        report_info.reporter_stake,
                        report_info.against_committee.clone(),
                        MTReporterSlashReason::ReportRefused,
                    );
                    for a_committee in report_info.support_committee {
                        let committee_ops = Self::committee_ops(&a_committee, report_id);
                        <T as pallet::Config>::ManageCommittee::add_slash(
                            a_committee.clone(),
                            committee_ops.staked_balance,
                            report_info.against_committee.clone(),
                            committee::CMSlashReason::MCInconsistentSubmit,
                        );
                    }
                }

                // If report_info.confirmed_committee.len() == 0 do nothing but clean
                Self::clean_finished_order(report_id);

                continue
            }
            // 当大于等于5分钟或者hashed的委员会已经达到3人，则更改报告状态，允许提交原始值
            if now - report_info.first_book_time >= 10u32.into() || report_info.hashed_committee.len() == 3 {
                if let ReportStatus::WaitingBook = report_info.report_status {
                    report_info.report_status = ReportStatus::SubmittingRaw;
                    ReportInfo::<T>::insert(report_id, report_info);
                }
                continue
            }
        }

        Ok(())
    }

    fn summary_fault_case() -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let mut live_report = Self::live_report();
        let mut live_report_is_changed = false;

        // 需要检查的report可能是正在被委员会验证/仍然可以预订的状态
        let mut verifying_report = live_report.verifying_report.clone();
        verifying_report.extend(live_report.bookable_report.clone());
        let submitting_raw_report = live_report.waiting_raw_report.clone();

        for a_report in verifying_report {
            let mut report_info = Self::report_info(&a_report);

            if let MachineFaultType::RentedInaccessible(..) = report_info.machine_fault_type {
                continue
            };

            // 不足验证时间截止时，处理：
            // 1. 报告人没有在规定时间内提交加密信息
            // 2. 委员会没有在1个小时内提交Hash
            if now - report_info.first_book_time < THREE_HOUR.saturated_into::<T::BlockNumber>() {
                if let ReportStatus::WaitingBook = report_info.report_status {
                    continue
                }

                let verifying_committee = report_info.verifying_committee.ok_or(())?;
                let committee_ops = Self::committee_ops(&verifying_committee, &a_report);

                // 报告人没有提交给原始信息，则惩罚报告人到国库，不进行奖励
                if committee_ops.encrypted_err_info.is_none() &&
                    now - committee_ops.booked_time >= HALF_HOUR.saturated_into::<T::BlockNumber>()
                {
                    Self::add_slash(
                        report_info.reporter,
                        report_info.reporter_stake,
                        Vec::new(),
                        MTReporterSlashReason::NotSubmitEncryptedInfo,
                    );
                    Self::refund_committee_clean_report(a_report);
                    continue
                }

                // 不足3小时，且委员会没有提交Hash，删除该委员会，并惩罚
                if now - committee_ops.booked_time >= ONE_HOUR.saturated_into::<T::BlockNumber>() {
                    report_info.verifying_committee = None;
                    if let Ok(index) = report_info.booked_committee.binary_search(&verifying_committee) {
                        report_info.booked_committee.remove(index);
                    }
                    if let Ok(index) = report_info.get_encrypted_info_committee.binary_search(&verifying_committee) {
                        report_info.get_encrypted_info_committee.remove(index);
                    }

                    // 如果此时booked_committee.len() == 0；返回到最初始的状态，并允许取消报告
                    if report_info.booked_committee.len() == 0 {
                        report_info.first_book_time = Zero::zero();
                        report_info.confirm_start = Zero::zero();
                        report_info.report_status = ReportStatus::Reported;
                    } else {
                        report_info.report_status = ReportStatus::WaitingBook
                    };

                    MTLiveReportList::rm_report_id(&mut live_report.verifying_report, a_report);
                    MTLiveReportList::add_report_id(&mut live_report.bookable_report, a_report);
                    live_report_is_changed = true;

                    // slash committee
                    <T as pallet::Config>::ManageCommittee::add_slash(
                        verifying_committee.clone(),
                        committee_ops.staked_balance,
                        Vec::new(),
                        committee::CMSlashReason::MCNotSubmitHash,
                    );

                    let mut committee_order = Self::committee_order(&verifying_committee);
                    if let Ok(index) = committee_order.booked_report.binary_search(&a_report) {
                        committee_order.booked_report.remove(index);
                    }

                    CommitteeOrder::<T>::insert(&verifying_committee, committee_order);
                    ReportInfo::<T>::insert(a_report, report_info);
                    CommitteeOps::<T>::remove(&verifying_committee, &a_report);

                    continue
                }
            }
            // 已经到3个小时
            else {
                if let ReportStatus::WaitingBook = report_info.report_status {
                    report_info.report_status = ReportStatus::SubmittingRaw;

                    MTLiveReportList::rm_report_id(&mut live_report.verifying_report, a_report);
                    MTLiveReportList::rm_report_id(&mut live_report.bookable_report, a_report);
                    MTLiveReportList::add_report_id(&mut live_report.waiting_raw_report, a_report);
                    live_report_is_changed = true;

                    ReportInfo::<T>::insert(a_report, report_info);
                    continue
                }

                // 但是最后一个委员会订阅时间小于1个小时
                let verifying_committee = report_info.verifying_committee.ok_or(())?;
                let committee_ops = Self::committee_ops(&verifying_committee, &a_report);

                if now - committee_ops.booked_time < ONE_HOUR.saturated_into::<T::BlockNumber>() {
                    // 将最后一个委员会移除，并不惩罚
                    report_info.verifying_committee = None;
                    if let Ok(index) = report_info.booked_committee.binary_search(&verifying_committee) {
                        report_info.booked_committee.remove(index);
                    }
                    if let Ok(index) = report_info.get_encrypted_info_committee.binary_search(&verifying_committee) {
                        report_info.get_encrypted_info_committee.remove(index);
                    }
                    report_info.report_status = ReportStatus::SubmittingRaw;

                    // 从最后一个委员会的存储中删除
                    Self::clean_from_committee_order(&verifying_committee, &a_report);
                    // 退还质押
                    let _ = T::ManageCommittee::change_used_stake(
                        verifying_committee.clone(),
                        committee_ops.staked_balance,
                        false,
                    );

                    MTLiveReportList::rm_report_id(&mut live_report.verifying_report, a_report);
                    MTLiveReportList::rm_report_id(&mut live_report.bookable_report, a_report);
                    MTLiveReportList::add_report_id(&mut live_report.waiting_raw_report, a_report);
                    live_report_is_changed = true;

                    CommitteeOps::<T>::remove(&verifying_committee, a_report);
                    ReportInfo::<T>::insert(a_report, report_info);

                    continue
                }
            }
        }

        // 正在提交原始值的
        for a_report in submitting_raw_report {
            Self::summary_waiting_raw(a_report, &mut live_report);
            live_report_is_changed = true;
        }

        if live_report_is_changed {
            LiveReport::<T>::put(live_report);
        }
        Ok(())
    }

    fn summary_waiting_raw(a_report: ReportId, live_report: &mut MTLiveReportList) {
        let now = <frame_system::Module<T>>::block_number();
        let mut report_info = Self::report_info(&a_report);

        // 未全部提交了原始信息且未达到了四个小时
        if now - report_info.report_time < FOUR_HOUR.saturated_into::<T::BlockNumber>() &&
            report_info.hashed_committee.len() != report_info.confirmed_committee.len()
        {
            return
        }
        match Self::summary_report(a_report) {
            ReportConfirmStatus::Confirmed(support_committees, against_committee, _) => {
                // Slash against_committee and release support committee stake
                for a_committee in against_committee.clone() {
                    let committee_ops = Self::committee_ops(&a_committee, a_report);
                    T::ManageCommittee::add_slash(
                        report_info.reporter.clone(),
                        committee_ops.staked_balance,
                        Vec::new(),
                        committee::CMSlashReason::MCInconsistentSubmit,
                    );
                }
                for a_committee in support_committees.clone() {
                    let committee_ops = Self::committee_ops(&a_committee, a_report);
                    let _ =
                        T::ManageCommittee::change_used_stake(a_committee.clone(), committee_ops.staked_balance, false);
                }

                MTLiveReportList::rm_report_id(&mut live_report.waiting_raw_report, a_report);
                MTLiveReportList::add_report_id(&mut live_report.finished_report, a_report);

                // 根据错误类型，调用不同的处理函数
                let fault_type = match report_info.machine_fault_type {
                    MachineFaultType::RentedInaccessible(..) =>
                        online_profile::OPSlashReason::RentedInaccessible(report_info.report_time),
                    MachineFaultType::RentedHardwareMalfunction(..) =>
                        online_profile::OPSlashReason::RentedHardwareMalfunction(report_info.report_time),
                    MachineFaultType::RentedHardwareCounterfeit(..) =>
                        online_profile::OPSlashReason::RentedHardwareCounterfeit(report_info.report_time),
                    MachineFaultType::OnlineRentFailed(..) =>
                        online_profile::OPSlashReason::OnlineRentFailed(report_info.report_time),
                };
                T::MTOps::mt_machine_offline(
                    report_info.reporter.clone(),
                    support_committees,
                    report_info.machine_id.clone(),
                    fault_type,
                );
            },
            ReportConfirmStatus::Refuse(support_committee, against_committee) => {
                // Slash support committee and release against committee stake
                for a_committee in support_committee {
                    let committee_ops = Self::committee_ops(&a_committee, a_report);
                    T::ManageCommittee::add_slash(
                        a_committee,
                        committee_ops.staked_balance,
                        against_committee.clone(),
                        committee::CMSlashReason::MCInconsistentSubmit,
                    );
                }
                for a_committee in against_committee.clone() {
                    let committee_ops = Self::committee_ops(&a_committee, a_report);
                    let _ =
                        T::ManageCommittee::change_used_stake(a_committee.clone(), committee_ops.staked_balance, false);
                }

                // Slash reporter
                Self::add_slash(
                    report_info.reporter.clone(),
                    report_info.reporter_stake,
                    against_committee,
                    MTReporterSlashReason::ReportRefused,
                );
            },
            // No consensus, will clean record & as new report to handle
            // In this case, no raw info is submitted, so committee record should be None
            ReportConfirmStatus::NoConsensus => {
                report_info.report_status = ReportStatus::Reported;
                // 仅在没有人提交原始值时才无共识，因此所有booked_committee都应该被惩罚
                for a_committee in report_info.booked_committee.clone() {
                    // clean from committee storage
                    CommitteeOps::<T>::remove(&a_committee, a_report);

                    // 从committee_order中删除
                    let mut committee_order = Self::committee_order(&a_committee);
                    if let Ok(index) = committee_order.booked_report.binary_search(&a_report) {
                        committee_order.booked_report.remove(index);
                    }
                    if let Ok(index) = committee_order.hashed_report.binary_search(&a_report) {
                        committee_order.hashed_report.remove(index);
                    }
                    CommitteeOrder::<T>::insert(&a_committee, committee_order);

                    let committee_ops = Self::committee_ops(&a_committee, a_report);
                    T::ManageCommittee::add_slash(
                        a_committee,
                        committee_ops.staked_balance,
                        vec![],
                        committee::CMSlashReason::MCNotSubmitRaw,
                    );
                }

                // All info of report should be cleaned, and so allow report be booked or cancled
                report_info = MTReportInfoDetail {
                    reporter: report_info.reporter,
                    report_time: report_info.report_time,
                    reporter_stake: report_info.reporter_stake,
                    report_status: ReportStatus::Reported,
                    machine_fault_type: report_info.machine_fault_type,
                    ..Default::default()
                };

                // 放到live_report的bookable字段
                MTLiveReportList::rm_report_id(&mut live_report.waiting_raw_report, a_report);
                MTLiveReportList::rm_report_id(&mut live_report.verifying_report, a_report);
                MTLiveReportList::add_report_id(&mut live_report.bookable_report, a_report);
            },
        }

        // FIXME: 在NoConsensus不能调用该方法
        Self::clean_finished_order(a_report);
        ReportInfo::<T>::insert(a_report, report_info);
    }
}
