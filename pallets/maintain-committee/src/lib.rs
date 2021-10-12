#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{alloc::string::ToString, Decode, Encode};
use frame_support::{
    pallet_prelude::*,
    traits::{Currency, OnUnbalanced, ReservableCurrency},
    IterableStorageMap,
};
use frame_system::pallet_prelude::*;
use generic_func::ItemList;
use online_profile_machine::{GNOps, MTOps, ManageCommittee};
use sp_io::hashing::blake2_128;
use sp_runtime::{
    traits::{CheckedAdd, CheckedSub, Zero},
    Perbill, RuntimeDebug,
};
use sp_std::{prelude::*, str, vec::Vec};

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

const FIVE_MINUTE: u32 = 10;
const TEN_MINUTE: u32 = 20;
const HALF_HOUR: u32 = 60;
const ONE_HOUR: u32 = 120;
const THREE_HOUR: u32 = 360;
const FOUR_HOUR: u32 = 480;
const TWO_DAY: u32 = 5760;

// pub type SlashId = u64;
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

/// 报告人的报告记录
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct ReporterReportList {
    pub processing_report: Vec<ReportId>,
    pub canceled_report: Vec<ReportId>,
    pub succeed_report: Vec<ReportId>,
    pub failed_report: Vec<ReportId>,
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
pub struct MTCommitteeOrderList {
    /// 委员会预订的报告
    pub booked_report: Vec<ReportId>,
    /// 已经提交了Hash信息的报告
    pub hashed_report: Vec<ReportId>,
    /// 已经提交了原始确认数据的报告
    pub confirmed_report: Vec<ReportId>,
    /// 已经成功上线的机器
    pub finished_report: Vec<ReportId>,
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
    /// How much stake will be used each report & how much should stake in this
    /// module to apply for SlashReview(reporter, committee, stash stake the same)
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

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTReportResultInfo<AccountId, BlockNumber, Balance> {
    pub report_id: ReportId,
    pub reporter: AccountId,
    pub reporter_stake: Balance,

    pub inconsistent_committee: Vec<AccountId>,
    pub unruly_committee: Vec<AccountId>,
    pub reward_committee: Vec<AccountId>,
    pub committee_stake: Balance,

    // TODO: add machine slash info
    pub machine_stash: AccountId,
    pub machine_id: MachineId,
    pub stash_slash_amount: Balance, // TODO: if should add this

    pub slash_time: BlockNumber,
    pub slash_exec_time: BlockNumber,

    pub report_result: ReportResultType,
    pub slash_result: MCSlashResult,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum ReportResultType {
    ReportSucceed,
    ReportRefused,
    ReporterNotSubmitEncryptedInfo,
    NoConsensus,
}

impl Default for ReportResultType {
    fn default() -> Self {
        Self::ReportRefused
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MCSlashResult {
    Pending,
    Canceled,
    Executed,
}

impl Default for MCSlashResult {
    fn default() -> Self {
        Self::Pending
    }
}

impl<AccountId, BlockNumber, Balance> MTReportResultInfo<AccountId, BlockNumber, Balance>
where
    AccountId: Ord,
{
    fn is_slashed_reporter(&self, who: &AccountId) -> bool {
        match self.report_result {
            ReportResultType::ReportRefused | ReportResultType::ReporterNotSubmitEncryptedInfo => &self.reporter == who,
            _ => false,
        }
    }

    fn is_slashed_committee(&self, who: &AccountId) -> bool {
        self.inconsistent_committee.binary_search(who).is_ok() || self.unruly_committee.binary_search(who).is_ok()
    }

    fn is_slashed_stash(&self, who: &AccountId) -> bool {
        match self.report_result {
            ReportResultType::ReportSucceed => &self.machine_stash == who,
            _ => false,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTPendingSlashReviewInfo<AccountId, Balance, BlockNumber> {
    pub applicant: AccountId,
    pub staked_amount: Balance,
    pub apply_time: BlockNumber,
    pub expire_time: BlockNumber,
    pub reason: Vec<u8>,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + generic_func::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type ManageCommittee: ManageCommittee<AccountId = Self::AccountId, Balance = BalanceOf<Self>>;
        type MTOps: MTOps<
            AccountId = Self::AccountId,
            MachineId = MachineId,
            FaultType = online_profile::OPSlashReason<Self::BlockNumber>,
            Balance = BalanceOf<Self>,
        >;
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
        type CancelSlashOrigin: EnsureOrigin<Self::Origin>;
        type SlashAndReward: GNOps<AccountId = Self::AccountId, Balance = BalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_n: BlockNumberFor<T>) -> frame_support::weights::Weight {
            let _ = Self::check_and_exec_pending_review();
            let _ = Self::check_and_exec_slash();
            0
        }

        fn on_finalize(_block_number: T::BlockNumber) {
            let _ = Self::summary_fault_case();
            let _ = Self::summary_offline_case();
        }
    }

    #[pallet::type_value]
    pub(super) fn CommitteeLimitDefault<T: Config>() -> u32 {
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
        StorageMap<_, Blake2_128Concat, T::AccountId, ReporterReportList, ValueQuery>;

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
        StorageMap<_, Blake2_128Concat, T::AccountId, MTCommitteeOrderList, ValueQuery>;

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
    #[pallet::getter(fn report_result)]
    pub(super) type ReportResult<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        ReportId,
        MTReportResultInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn unhandled_report_result)]
    pub(super) type UnhandledReportResult<T: Config> = StorageValue<_, Vec<ReportId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pending_slash_review)]
    pub(super) type PendingSlashReview<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        ReportId,
        MTPendingSlashReviewInfo<T::AccountId, BalanceOf<T>, T::BlockNumber>,
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

            ensure!(<T as Config>::Currency::can_reserve(&reporter, amount), Error::<T>::BalanceNotEnough);
            reporter_stake.staked_amount += amount;

            ensure!(
                reporter_stake.staked_amount - reporter_stake.used_stake >
                    stake_params.min_free_stake_percent * reporter_stake.staked_amount,
                Error::<T>::StakeNotEnough
            );

            <T as pallet::Config>::Currency::reserve(&reporter, amount).map_err(|_| Error::<T>::BalanceNotEnough)?;
            ReporterStake::<T>::insert(&reporter, reporter_stake);
            Self::deposit_event(Event::ReporterAddStake(reporter, amount));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn reporter_reduce_stake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            let stake_params = Self::reporter_stake_params().ok_or(Error::<T>::GetStakeAmountFailed)?;
            let mut reporter_stake = Self::reporter_stake(&reporter);

            ensure!(reporter_stake.staked_amount >= amount, Error::<T>::BalanceNotEnough);
            reporter_stake.staked_amount -= amount;

            ensure!(
                reporter_stake.staked_amount - reporter_stake.used_stake >
                    stake_params.min_free_stake_percent * reporter_stake.staked_amount,
                Error::<T>::StakeNotEnough
            );

            <T as pallet::Config>::Currency::unreserve(&reporter, amount);
            ReporterStake::<T>::insert(&reporter, reporter_stake);

            Self::deposit_event(Event::ReporterReduceStake(reporter, amount));
            Ok(().into())
        }

        // 报告人可以在抢单之前取消该报告
        #[pallet::weight(10000)]
        pub fn reporter_cancel_report(origin: OriginFor<T>, report_id: ReportId) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;

            let report_info = Self::report_info(&report_id);
            ensure!(report_info.report_status == ReportStatus::Reported, Error::<T>::OrderNotAllowCancel);
            ensure!(report_info.reporter == reporter, Error::<T>::NotReporter);

            // 清理存储
            let mut live_report = Self::live_report();
            ItemList::rm_item(&mut live_report.bookable_report, &report_id);

            let mut reporter_report = Self::reporter_report(&reporter);
            ItemList::rm_item(&mut reporter_report.processing_report, &report_id);
            ItemList::add_item(&mut reporter_report.canceled_report, report_id);

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
        pub fn committee_book_report(origin: OriginFor<T>, report_id: ReportId) -> DispatchResultWithPostInfo {
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
            ensure!(report_info.booked_committee.binary_search(&committee).is_err(), Error::<T>::AlreadyBooked);

            ItemList::add_item(&mut report_info.booked_committee, committee.clone());
            // 记录第一个预订订单的时间, 3个小时(360个块)之后开始提交原始值
            if report_info.booked_committee.len() == 1 {
                report_info.first_book_time = now;
                report_info.confirm_start = now + THREE_HOUR.into();
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
                        ItemList::rm_item(&mut live_report.bookable_report, &report_id);
                        ItemList::add_item(&mut live_report.verifying_report, report_id);

                        is_live_report_changed = true;
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
                    ItemList::rm_item(&mut live_report.bookable_report, &report_id);
                    ItemList::add_item(&mut live_report.verifying_report, report_id);

                    is_live_report_changed = true;
                },
            }

            // 记录当前哪个委员会正在验证，方便状态控制
            report_info.verifying_committee = Some(committee.clone());

            // 添加到委员会自己的存储中
            let mut committee_order = Self::committee_order(&committee);
            ItemList::add_item(&mut committee_order.booked_report, report_id);

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
            ensure!(report_info.booked_committee.binary_search(&to_committee).is_ok(), Error::<T>::NotOrderCommittee);

            // report_info中插入已经收到了加密信息的委员会
            ItemList::add_item(&mut report_info.get_encrypted_info_committee, to_committee.clone());

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
        pub fn committee_submit_verify_hash(
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

            ensure!(committee_order.booked_report.binary_search(&report_id).is_ok(), Error::<T>::NotInBookedList);

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
            ItemList::add_item(&mut report_info.hashed_committee, committee.clone());

            // 判断是否已经有3个了
            if report_info.hashed_committee.len() == committee_limit as usize {
                // 满足要求的Hash已镜提交，则进入提交raw的阶段
                ItemList::rm_item(&mut live_report.verifying_report, &report_id);
                ItemList::add_item(&mut live_report.waiting_raw_report, report_id);

                report_info.report_status = ReportStatus::SubmittingRaw;
            } else {
                ItemList::rm_item(&mut live_report.verifying_report, &report_id);
                ItemList::add_item(&mut live_report.bookable_report, report_id);

                report_info.report_status = ReportStatus::WaitingBook;
            }

            report_info.verifying_committee = None;

            // 修改committeeOps存储/状态
            committee_ops.order_status = MTOrderStatus::WaitingRaw;
            committee_ops.confirm_hash = hash;
            committee_ops.hash_time = now;

            // 将订单从委员会已预订移动到已Hash
            ItemList::rm_item(&mut committee_order.booked_report, &report_id);
            ItemList::add_item(&mut committee_order.hashed_report, report_id);

            LiveReport::<T>::put(live_report);
            CommitteeOps::<T>::insert(&committee, &report_id, committee_ops);
            CommitteeOrder::<T>::insert(&committee, committee_order);
            ReportInfo::<T>::insert(&report_id, report_info);

            Self::deposit_event(Event::HashSubmited(report_id, committee));
            Ok(().into())
        }

        /// 订单状态必须是等待SubmittingRaw: 除了offline之外的所有错误类型
        #[pallet::weight(10000)]
        pub fn committee_submit_verify_raw(
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
                MachineFaultType::RentedHardwareMalfunction(hash, _) |
                MachineFaultType::RentedHardwareCounterfeit(hash, _) |
                MachineFaultType::OnlineRentFailed(hash, _) => hash,
                MachineFaultType::RentedInaccessible(..) => return Err(Error::<T>::OrderStatusNotFeat.into()),
            };

            // 检查是否提交了该订单的hash
            ensure!(report_info.hashed_committee.binary_search(&committee).is_ok(), Error::<T>::NotProperCommittee);
            // 添加到Report的已提交Raw的列表
            ItemList::add_item(&mut report_info.confirmed_committee, committee.clone());

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
                ItemList::add_item(&mut report_info.support_committee, committee.clone());
            } else {
                ItemList::add_item(&mut report_info.against_committee, committee.clone());
            }

            report_info.machine_id = machine_id;
            report_info.err_info = err_reason;
            committee_ops = MTCommitteeOpsDetail {
                confirm_time: now,
                confirm_result: support_report,
                extra_err_info,
                order_status: MTOrderStatus::Finished,
                ..committee_ops
            };

            CommitteeOps::<T>::insert(&committee, &report_id, committee_ops);
            ReportInfo::<T>::insert(&report_id, report_info);

            Self::deposit_event(Event::RawInfoSubmited(report_id, committee));
            Ok(().into())
        }

        // TODO: 与 committee_submit_confirm_raw合并
        #[pallet::weight(10000)]
        pub fn committee_submit_offline_raw(
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
            ensure!(report_info.hashed_committee.binary_search(&committee).is_ok(), Error::<T>::NotProperCommittee);

            // 添加到Report的已提交Raw的列表
            ItemList::add_item(&mut report_info.confirmed_committee, committee.clone());

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
                ItemList::add_item(&mut report_info.support_committee, committee.clone());
            } else {
                ItemList::add_item(&mut report_info.against_committee, committee.clone());
            }

            committee_ops.confirm_time = now;
            committee_ops.confirm_result = is_support;

            CommitteeOps::<T>::insert(&committee, &report_id, committee_ops);
            ReportInfo::<T>::insert(&report_id, report_info);

            Self::deposit_event(Event::RawInfoSubmited(report_id, committee));
            Ok(().into())
        }

        // TODO: allow machine stash apply
        /// Reporter and committee apply technical committee review
        #[pallet::weight(10000)]
        pub fn apply_slash_review(
            origin: OriginFor<T>,
            report_result_id: ReportId,
            reason: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let applicant = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let reporter_stake_params = Self::reporter_stake_params().ok_or(Error::<T>::GetStakeAmountFailed)?;
            // let committee_order_stake =
            //     T::ManageCommittee::stake_per_order().ok_or(Error::<T>::GetStakeAmountFailed)?;

            let report_result_info = Self::report_result(report_result_id);
            let is_slashed_reporter = report_result_info.is_slashed_reporter(&applicant);
            let is_slashed_committee = report_result_info.is_slashed_committee(&applicant);
            let is_slashed_stash = report_result_info.is_slashed_stash(&applicant);

            ensure!(!PendingSlashReview::<T>::contains_key(report_result_id), Error::<T>::AlreadyApplied);
            ensure!(is_slashed_reporter || is_slashed_committee || is_slashed_stash, Error::<T>::NotSlashed);
            ensure!(now < report_result_info.slash_exec_time, Error::<T>::TimeNotAllowed);

            ensure!(
                <T as Config>::Currency::can_reserve(&applicant, reporter_stake_params.stake_per_report),
                Error::<T>::BalanceNotEnough
            );

            // Add stake when apply for review
            // NOTE: here, should add total stake not add used stake
            if is_slashed_reporter {
                let mut reporter_stake = Self::reporter_stake(&applicant);
                reporter_stake.staked_amount = reporter_stake
                    .staked_amount
                    .checked_add(&reporter_stake_params.stake_per_report)
                    .ok_or(Error::<T>::BalanceNotEnough)?;
                ensure!(
                    reporter_stake.staked_amount - reporter_stake.used_stake >
                        reporter_stake_params.min_free_stake_percent * reporter_stake.staked_amount,
                    Error::<T>::StakeNotEnough
                );
                <T as pallet::Config>::Currency::reserve(&applicant, reporter_stake_params.stake_per_report)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;
                ReporterStake::<T>::insert(&applicant, reporter_stake);
            } else if is_slashed_committee {
                // Change committee stake
                <T as pallet::Config>::ManageCommittee::change_total_stake(
                    applicant.clone(),
                    reporter_stake_params.stake_per_report,
                    true,
                )
                .map_err(|_| Error::<T>::BalanceNotEnough)?;

                <T as pallet::Config>::Currency::reserve(&applicant, reporter_stake_params.stake_per_report)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;
            } else if is_slashed_stash {
                // change stash stake
                T::MTOps::mt_change_staked_balance(applicant.clone(), reporter_stake_params.stake_per_report, true)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;
            }

            PendingSlashReview::<T>::insert(
                report_result_id,
                MTPendingSlashReviewInfo {
                    applicant,
                    staked_amount: reporter_stake_params.stake_per_report,
                    apply_time: now,
                    expire_time: report_result_info.slash_exec_time,
                    reason,
                },
            );

            Self::deposit_event(Event::ApplySlashReview(report_result_id));
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn cancel_reporter_slash(origin: OriginFor<T>, slashed_report_id: ReportId) -> DispatchResultWithPostInfo {
            T::CancelSlashOrigin::ensure_origin(origin)?;
            ensure!(ReportResult::<T>::contains_key(slashed_report_id), Error::<T>::SlashIdNotExist);
            ensure!(PendingSlashReview::<T>::contains_key(slashed_report_id), Error::<T>::NotPendingReviewSlash);

            let now = <frame_system::Module<T>>::block_number();
            let mut report_result_info = Self::report_result(slashed_report_id);
            let slash_review_info = Self::pending_slash_review(slashed_report_id);

            ensure!(slash_review_info.expire_time > now, Error::<T>::ExpiredApply);

            let is_slashed_reporter = report_result_info.is_slashed_reporter(&slash_review_info.applicant);
            // let is_slashed_committee = report_result_info.is_slashed_committee(&slash_review_info.applicant);

            // Return reserved balance when apply for review
            if is_slashed_reporter {
                let _ =
                    Self::change_reporter_stake(&slash_review_info.applicant, slash_review_info.staked_amount, false);
            } else {
                let _ = Self::change_committee_stake(
                    vec![slash_review_info.applicant],
                    slash_review_info.staked_amount,
                    false,
                );
            }

            // revert reward and slash
            let is_reporter_slashed = match report_result_info.report_result {
                ReportResultType::ReportRefused | ReportResultType::ReporterNotSubmitEncryptedInfo => true,
                _ => false,
            };

            let mut should_slash = report_result_info.reward_committee.clone();
            for a_committee in report_result_info.unruly_committee.clone() {
                ItemList::add_item(&mut should_slash, a_committee)
            }
            let mut should_reward = report_result_info.inconsistent_committee.clone();

            if is_reporter_slashed {
                let _ =
                    Self::change_reporter_stake(&report_result_info.reporter, report_result_info.reporter_stake, false);

                ItemList::add_item(&mut should_reward, report_result_info.reporter.clone());
            } else {
                let _ =
                    Self::change_reporter_stake(&report_result_info.reporter, report_result_info.reporter_stake, true);

                // slash reporter
                let _ = T::SlashAndReward::slash_and_reward(
                    vec![report_result_info.reporter.clone()],
                    report_result_info.reporter_stake,
                    should_reward.clone(),
                );
            }

            let _ = T::SlashAndReward::slash_and_reward(
                should_slash,
                report_result_info.committee_stake,
                should_reward.clone(),
            );

            // remove from unhandled report result
            let mut unhandled_report_result = Self::unhandled_report_result();
            ItemList::rm_item(&mut unhandled_report_result, &slashed_report_id);

            report_result_info.slash_result = MCSlashResult::Canceled;

            UnhandledReportResult::<T>::put(unhandled_report_result);
            ReportResult::<T>::insert(slashed_report_id, report_result_info);
            PendingSlashReview::<T>::remove(slashed_report_id);
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
        ReporterAddStake(T::AccountId, BalanceOf<T>),
        ReporterReduceStake(T::AccountId, BalanceOf<T>),
        ApplySlashReview(ReportId),
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
        NotReporter,
        TimeNotAllowed,
        SlashIdNotExist,
        NotPendingReviewSlash,
        NotSlashed,
        AlreadyApplied,
        ExpiredApply,
    }
}

impl<T: Config> Pallet<T> {
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

        // 各种报告类型，都需要质押 1000 DBC
        // 如果是第一次绑定，则需要质押2w DBC，其他情况:
        if reporter_stake.staked_amount == Zero::zero() {
            ensure!(
                <T as Config>::Currency::can_reserve(&reporter, stake_params.stake_baseline),
                Error::<T>::BalanceNotEnough
            );

            <T as pallet::Config>::Currency::reserve(&reporter, stake_params.stake_baseline)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;
            reporter_stake.staked_amount = stake_params.stake_baseline;
            reporter_stake.used_stake = stake_params.stake_per_report;
        } else {
            reporter_stake.used_stake += stake_params.stake_per_report;
            ensure!(
                reporter_stake.staked_amount - reporter_stake.used_stake >=
                    stake_params.min_free_stake_percent * reporter_stake.staked_amount,
                Error::<T>::StakeNotEnough
            );
        }

        let mut live_report = Self::live_report();
        let mut reporter_report = Self::reporter_report(&reporter);

        // Record to live_report & reporter_report
        ItemList::add_item(&mut live_report.bookable_report, report_id);
        ItemList::add_item(&mut reporter_report.processing_report, report_id);

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
        report_id
    }

    fn get_hash(raw_str: &Vec<u8>) -> [u8; 16] {
        blake2_128(raw_str)
    }

    fn rm_from_committee_order(committee_order: &mut MTCommitteeOrderList, report_id: &ReportId) {
        ItemList::rm_item(&mut committee_order.booked_report, report_id);
        ItemList::rm_item(&mut committee_order.hashed_report, report_id);
        ItemList::rm_item(&mut committee_order.confirmed_report, report_id);
    }

    fn rm_from_live_report(live_report: &mut MTLiveReportList, report_id: &ReportId) {
        ItemList::rm_item(&mut live_report.bookable_report, report_id);
        ItemList::rm_item(&mut live_report.verifying_report, report_id);
        ItemList::rm_item(&mut live_report.waiting_raw_report, report_id);
    }

    // Summary committee's handle result depend on support & against votes
    fn summary_report(report_id: ReportId) -> ReportConfirmStatus<T::AccountId> {
        let report_info = Self::report_info(&report_id);

        if report_info.confirmed_committee.len() == 0 {
            return ReportConfirmStatus::NoConsensus
        }

        if report_info.support_committee.len() >= report_info.against_committee.len() {
            return ReportConfirmStatus::Confirmed(
                report_info.support_committee,
                report_info.against_committee,
                report_info.err_info,
            )
        }
        ReportConfirmStatus::Refuse(report_info.support_committee, report_info.against_committee)
    }

    // Slash offline machine
    fn summary_offline_case() -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let mut live_report = Self::live_report();
        let mut verifying_report = live_report.verifying_report.clone();
        verifying_report.extend(live_report.bookable_report.clone());
        let committee_order_stake = T::ManageCommittee::stake_per_order().unwrap_or_default();

        for report_id in verifying_report {
            let mut report_info = Self::report_info(&report_id);
            let mut reporter_report = Self::reporter_report(&report_info.reporter);

            // 仅处理Offline的情况
            match report_info.machine_fault_type {
                MachineFaultType::RentedInaccessible(..) => {},
                _ => continue,
            }

            match report_info.report_status {
                ReportStatus::Reported | ReportStatus::CommitteeConfirmed => continue,
                ReportStatus::WaitingBook | ReportStatus::Verifying => {
                    // 当大于等于5分钟或者hashed的委员会已经达到3人，则更改报告状态，允许提交原始值
                    if now - report_info.first_book_time >= FIVE_MINUTE.into() ||
                        report_info.hashed_committee.len() == 3
                    {
                        report_info.report_status = ReportStatus::SubmittingRaw;
                        ReportInfo::<T>::insert(report_id, report_info);
                    }
                    continue
                },
                ReportStatus::SubmittingRaw => {
                    if now - report_info.first_book_time < TEN_MINUTE.into() &&
                        report_info.confirmed_committee.len() < report_info.hashed_committee.len()
                    {
                        continue
                    }
                },
            }

            let mut report_result = Self::report_result(report_id);
            // 此时，应该否决报告人，处理委员会
            report_result = MTReportResultInfo {
                report_id,
                reporter: report_info.reporter.clone(),
                reporter_stake: report_info.reporter_stake,
                committee_stake: committee_order_stake,
                slash_time: now,
                slash_exec_time: now + TWO_DAY.into(),
                slash_result: MCSlashResult::Pending,
                ..report_result
            };

            // 当大于等于10分钟，或者提交确认的委员会等于提交了hash的委员会，需要执行后面的逻辑，来确认
            // 统计预订了但没有提交确认的委员会
            for a_committee in report_info.booked_committee {
                let mut committee_order = Self::committee_order(&a_committee);

                if report_info.confirmed_committee.binary_search(&a_committee).is_ok() {
                    ItemList::add_item(&mut committee_order.finished_report, report_id);
                } else {
                    ItemList::add_item(&mut &mut report_result.unruly_committee, a_committee.clone());
                }

                CommitteeOps::<T>::remove(&a_committee, report_id);
                Self::rm_from_committee_order(&mut committee_order, &report_id);
                CommitteeOrder::<T>::insert(&a_committee, committee_order);
            }

            // 无共识：未提交确认值的惩罚已经在前面执行了，需要将该报告重置，并允许再次抢单
            if report_info.confirmed_committee.len() == 0 {
                report_info = MTReportInfoDetail {
                    reporter: report_info.reporter,
                    report_time: report_info.report_time,
                    reporter_stake: report_info.reporter_stake,
                    machine_id: report_info.machine_id,
                    report_status: ReportStatus::Reported,
                    machine_fault_type: report_info.machine_fault_type,
                    ..Default::default()
                };

                ItemList::rm_item(&mut live_report.verifying_report, &report_id);
                ItemList::add_item(&mut live_report.bookable_report, report_id);

                ReportInfo::<T>::insert(report_id, report_info);
                report_result.report_result = ReportResultType::NoConsensus;
                // Should do slash at once
                if report_result.unruly_committee.len() > 0 {
                    ReportResult::<T>::insert(report_id, report_result);
                    let mut unhandled_report_result = Self::unhandled_report_result();
                    ItemList::add_item(&mut unhandled_report_result, report_id);
                    UnhandledReportResult::<T>::put(unhandled_report_result);
                }
                continue
            }

            ItemList::rm_item(&mut reporter_report.processing_report, &report_id);
            if report_info.support_committee >= report_info.against_committee {
                // 此时，应该支持报告人，惩罚反对的委员会
                T::MTOps::mt_machine_offline(
                    report_info.reporter.clone(),
                    report_info.support_committee.clone(),
                    report_info.machine_id.clone(),
                    online_profile::OPSlashReason::RentedInaccessible(report_info.report_time),
                );
                for a_committee in report_info.against_committee {
                    ItemList::add_item(&mut report_result.inconsistent_committee, a_committee);
                }
                for a_committee in report_info.support_committee {
                    ItemList::add_item(&mut report_result.inconsistent_committee, a_committee);
                }

                ItemList::add_item(&mut reporter_report.succeed_report, report_id);
                report_result.report_result = ReportResultType::ReportSucceed;
            } else {
                for a_committee in report_info.support_committee {
                    ItemList::add_item(&mut report_result.inconsistent_committee, a_committee);
                }
                for a_committee in report_info.against_committee {
                    ItemList::add_item(&mut report_result.reward_committee, a_committee);
                }

                ItemList::add_item(&mut reporter_report.failed_report, report_id);

                report_result.report_result = ReportResultType::ReportRefused;
            }

            ReporterReport::<T>::insert(&report_info.reporter, reporter_report);

            // 支持或反对，该报告都变为完成状态
            Self::rm_from_live_report(&mut live_report, &report_id);
            ItemList::add_item(&mut live_report.finished_report, report_id);

            ReportResult::<T>::insert(report_id, report_result);
            let mut unhandled_report_result = Self::unhandled_report_result();
            ItemList::add_item(&mut unhandled_report_result, report_id);
            UnhandledReportResult::<T>::put(unhandled_report_result);
        }

        LiveReport::<T>::put(live_report);
        Ok(())
    }

    fn summary_fault_case() -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let mut live_report = Self::live_report();
        let mut live_report_is_changed = false;
        let committee_order_stake = T::ManageCommittee::stake_per_order().unwrap_or_default();

        // 需要检查的report可能是正在被委员会验证/仍然可以预订的状态
        let mut verifying_report = live_report.verifying_report.clone();
        verifying_report.extend(live_report.bookable_report.clone());
        let submitting_raw_report = live_report.waiting_raw_report.clone();

        for report_id in verifying_report {
            let mut report_info = Self::report_info(&report_id);
            // 忽略掉线的类型
            if let MachineFaultType::RentedInaccessible(..) = report_info.machine_fault_type {
                continue
            };

            let mut reporter_report = Self::reporter_report(&report_info.reporter);

            let mut report_result = Self::report_result(report_id);
            report_result = MTReportResultInfo {
                report_id,
                reporter: report_info.reporter.clone(),
                reporter_stake: report_info.reporter_stake,
                committee_stake: committee_order_stake,
                slash_time: now,
                slash_exec_time: now + TWO_DAY.into(),
                slash_result: MCSlashResult::Pending,

                ..report_result
            };

            // 不到验证截止时间时:
            if now - report_info.first_book_time < THREE_HOUR.into() {
                if let ReportStatus::WaitingBook = report_info.report_status {
                    continue
                }

                let verifying_committee = report_info.verifying_committee.ok_or(())?;
                let committee_ops = Self::committee_ops(&verifying_committee, &report_id);

                // 1. 报告人没有在规定时间内提交给加密信息，则惩罚报告人到国库，不进行奖励
                if committee_ops.encrypted_err_info.is_none() && now - committee_ops.booked_time >= HALF_HOUR.into() {
                    ItemList::rm_item(&mut reporter_report.processing_report, &report_id);
                    ItemList::add_item(&mut reporter_report.failed_report, report_id);
                    ReporterReport::<T>::insert(&report_info.reporter, reporter_report);

                    // 清理存储: CommitteeOps, LiveReport, CommitteeOrder, ReporterRecord
                    for a_committee in report_info.booked_committee {
                        let committee_ops = Self::committee_ops(&a_committee, &report_id);
                        let _ = <T as pallet::Config>::ManageCommittee::change_used_stake(
                            a_committee.clone(),
                            committee_ops.staked_balance,
                            false,
                        );
                        CommitteeOps::<T>::remove(&a_committee, report_id);

                        let mut committee_order = Self::committee_order(&a_committee);
                        Self::rm_from_committee_order(&mut committee_order, &report_id);
                        CommitteeOrder::<T>::insert(&a_committee, committee_order);
                    }

                    ItemList::rm_item(&mut live_report.verifying_report, &report_id);
                    live_report_is_changed = true;
                    report_result.report_result = ReportResultType::ReporterNotSubmitEncryptedInfo;
                    ReportResult::<T>::insert(report_id, report_result);
                    let mut unhandled_report_result = Self::unhandled_report_result();
                    ItemList::add_item(&mut unhandled_report_result, report_id);
                    UnhandledReportResult::<T>::put(unhandled_report_result);

                    continue
                }

                // 2. 委员会没有提交Hash，删除该委员会，并惩罚
                if now - committee_ops.booked_time >= ONE_HOUR.into() {
                    // 更改report_info
                    report_info.verifying_committee = None;
                    ItemList::rm_item(&mut report_info.booked_committee, &verifying_committee);
                    ItemList::rm_item(&mut report_info.get_encrypted_info_committee, &verifying_committee);

                    // 如果此时booked_committee.len() == 0；返回到最初始的状态，并允许取消报告
                    if report_info.booked_committee.len() == 0 {
                        report_info.first_book_time = Zero::zero();
                        report_info.confirm_start = Zero::zero();
                        report_info.report_status = ReportStatus::Reported;
                    } else {
                        report_info.report_status = ReportStatus::WaitingBook
                    };

                    ItemList::rm_item(&mut live_report.verifying_report, &report_id);
                    ItemList::add_item(&mut live_report.bookable_report, report_id);
                    live_report_is_changed = true;

                    let mut committee_order = Self::committee_order(&verifying_committee);
                    ItemList::rm_item(&mut committee_order.booked_report, &report_id);

                    CommitteeOrder::<T>::insert(&verifying_committee, committee_order);
                    ReportInfo::<T>::insert(report_id, report_info.clone());
                    CommitteeOps::<T>::remove(&verifying_committee, &report_id);

                    // NOTE: should not insert directly when summary result, but should alert exist data
                    ItemList::add_item(&mut report_result.unruly_committee, verifying_committee.clone());
                    ReportResult::<T>::insert(report_id, report_result);
                    let mut unhandled_report_result = Self::unhandled_report_result();
                    ItemList::add_item(&mut unhandled_report_result, report_id);
                    UnhandledReportResult::<T>::put(unhandled_report_result);
                    continue
                }
            }
            // 已经到3个小时
            else {
                Self::rm_from_live_report(&mut live_report, &report_id);
                ItemList::add_item(&mut live_report.waiting_raw_report, report_id);
                live_report_is_changed = true;

                if let ReportStatus::WaitingBook = report_info.report_status {
                    report_info.report_status = ReportStatus::SubmittingRaw;
                    ReportInfo::<T>::insert(report_id, report_info);
                    continue
                }

                // 但是最后一个委员会订阅时间小于1个小时
                let verifying_committee = report_info.verifying_committee.ok_or(())?;
                let committee_ops = Self::committee_ops(&verifying_committee, &report_id);

                if now - committee_ops.booked_time < ONE_HOUR.into() {
                    // 将最后一个委员会移除，不惩罚
                    report_info.verifying_committee = None;
                    ItemList::rm_item(&mut report_info.booked_committee, &verifying_committee);
                    ItemList::rm_item(&mut report_info.get_encrypted_info_committee, &verifying_committee);

                    report_info.report_status = ReportStatus::SubmittingRaw;

                    // 从最后一个委员会的存储中删除,并退还质押
                    let mut committee_order = Self::committee_order(&verifying_committee);
                    Self::rm_from_committee_order(&mut committee_order, &report_id);
                    CommitteeOrder::<T>::insert(&verifying_committee, committee_order);

                    let _ = T::ManageCommittee::change_used_stake(
                        verifying_committee.clone(),
                        committee_ops.staked_balance,
                        false,
                    );

                    CommitteeOps::<T>::remove(&verifying_committee, report_id);
                    ReportInfo::<T>::insert(report_id, report_info);

                    continue
                }
            }
        }

        // 正在提交原始值的
        for report_id in submitting_raw_report {
            live_report_is_changed = Self::summary_waiting_raw(report_id, &mut live_report) || live_report_is_changed;
        }

        if live_report_is_changed {
            LiveReport::<T>::put(live_report);
        }
        Ok(())
    }

    fn summary_waiting_raw(report_id: ReportId, live_report: &mut MTLiveReportList) -> bool {
        let now = <frame_system::Module<T>>::block_number();
        let mut report_info = Self::report_info(&report_id);
        let mut live_report_is_changed = false;

        let committee_order_stake = T::ManageCommittee::stake_per_order().unwrap_or_default();
        let mut report_result = Self::report_result(report_id);

        // 未全部提交了原始信息且未达到了四个小时
        if now - report_info.report_time < FOUR_HOUR.into() &&
            report_info.hashed_committee.len() != report_info.confirmed_committee.len()
        {
            return false
        }
        match Self::summary_report(report_id) {
            ReportConfirmStatus::Confirmed(support_committees, against_committee, _) => {
                // Slash against_committee and release support committee stake
                for a_committee in against_committee.clone() {
                    ItemList::add_item(&mut report_result.inconsistent_committee, a_committee.clone());

                    // 改变committee_order
                    let mut committee_order = Self::committee_order(&a_committee);
                    Self::rm_from_committee_order(&mut committee_order, &report_id);
                    CommitteeOrder::<T>::insert(&a_committee, committee_order);
                }
                for a_committee in support_committees.clone() {
                    ItemList::add_item(&mut report_result.reward_committee, a_committee.clone());

                    // 改变committee_order
                    let mut committee_order = Self::committee_order(&a_committee);
                    Self::rm_from_committee_order(&mut committee_order, &report_id);
                    ItemList::add_item(&mut committee_order.finished_report, report_id);
                    CommitteeOrder::<T>::insert(&a_committee, committee_order);
                }

                ItemList::rm_item(&mut live_report.waiting_raw_report, &report_id);
                ItemList::add_item(&mut live_report.finished_report, report_id);
                live_report_is_changed = true;

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
                for a_committee in support_committee.clone() {
                    ItemList::add_item(&mut report_result.inconsistent_committee, a_committee);
                }
                for a_committee in against_committee.clone() {
                    ItemList::add_item(&mut report_result.reward_committee, a_committee);
                }
            },
            // No consensus, will clean record & as new report to handle
            // In this case, no raw info is submitted, so committee record should be None
            ReportConfirmStatus::NoConsensus => {
                report_info.report_status = ReportStatus::Reported;
                // 仅在没有人提交原始值时才无共识，因此所有booked_committee都应该被惩罚
                for a_committee in report_info.booked_committee.clone() {
                    // clean from committee storage
                    CommitteeOps::<T>::remove(&a_committee, report_id);

                    // 从committee_order中删除
                    let mut committee_order = Self::committee_order(&a_committee);
                    ItemList::rm_item(&mut committee_order.booked_report, &report_id);
                    ItemList::rm_item(&mut committee_order.hashed_report, &report_id);
                    CommitteeOrder::<T>::insert(&a_committee, committee_order);
                }

                // All info of report should be cleaned, and so allow report be booked or canceled
                report_info = MTReportInfoDetail {
                    reporter: report_info.reporter,
                    report_time: report_info.report_time,
                    reporter_stake: report_info.reporter_stake,
                    report_status: ReportStatus::Reported,
                    machine_fault_type: report_info.machine_fault_type,
                    ..Default::default()
                };

                // 放到live_report的bookable字段
                ItemList::rm_item(&mut live_report.waiting_raw_report, &report_id);
                ItemList::rm_item(&mut live_report.verifying_report, &report_id);
                ItemList::add_item(&mut live_report.bookable_report, report_id);
                live_report_is_changed = true;
            },
        }

        if report_result.unruly_committee.len() > 0 || report_result.inconsistent_committee.len() > 0 {
            report_result = MTReportResultInfo {
                report_id,
                reporter: report_info.reporter.clone(),
                reporter_stake: report_info.reporter_stake,

                committee_stake: committee_order_stake,
                slash_time: now,
                slash_exec_time: now + TWO_DAY.into(),

                report_result: ReportResultType::ReportRefused,
                slash_result: MCSlashResult::Pending,

                ..report_result
            };

            ReportResult::<T>::insert(report_id, report_result);
            let mut unhandled_report_result = Self::unhandled_report_result();
            ItemList::add_item(&mut unhandled_report_result, report_id);
            UnhandledReportResult::<T>::put(unhandled_report_result);
        } else {
            let committee_order_stake = T::ManageCommittee::stake_per_order().unwrap_or_default();
            for a_committee in report_result.reward_committee {
                let _ = <T as pallet::Config>::ManageCommittee::change_used_stake(
                    a_committee.clone(),
                    committee_order_stake,
                    false,
                );
            }
        }

        ReportInfo::<T>::insert(report_id, report_info);
        live_report_is_changed
    }

    // if is_slash, reduce both used_stake & total_stake
    // else reduce only used_stake
    fn change_reporter_stake(reporter: &T::AccountId, amount: BalanceOf<T>, is_slash: bool) -> Result<(), ()> {
        let mut reporter_stake = Self::reporter_stake(reporter);
        reporter_stake.used_stake = reporter_stake.used_stake.checked_sub(&amount).ok_or(())?;

        if is_slash {
            reporter_stake.staked_amount = reporter_stake.staked_amount.checked_sub(&amount).ok_or(())?;
        }

        ReporterStake::<T>::insert(reporter, reporter_stake);
        Ok(())
    }

    fn change_committee_stake(
        committee_list: Vec<T::AccountId>,
        amount: BalanceOf<T>,
        is_slash: bool,
    ) -> Result<(), ()> {
        for a_committee in committee_list {
            if is_slash {
                <T as pallet::Config>::ManageCommittee::change_total_stake(a_committee.clone(), amount, false)?;
            }

            <T as pallet::Config>::ManageCommittee::change_used_stake(a_committee, amount, false)?;
        }

        Ok(())
    }

    // TODO: add interface to query from CommitteeModule if slash is canceled!
    fn check_and_exec_slash() -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let mut pending_unhandled_id = Self::unhandled_report_result();

        for slashed_report_id in pending_unhandled_id.clone() {
            let mut report_result_info = Self::report_result(&slashed_report_id);
            if now < report_result_info.slash_exec_time {
                continue
            }

            // TODO: refa code
            // TODO: add handler if applicant is stash
            match report_result_info.report_result {
                ReportResultType::ReportSucceed => {
                    // slash unruly & inconsistent, reward to reward_committee & reporter
                    let mut slash_who = report_result_info.unruly_committee.clone();
                    for a_inconsistent in report_result_info.inconsistent_committee.clone() {
                        ItemList::add_item(&mut slash_who, a_inconsistent);
                    }

                    let mut reward_who = report_result_info.reward_committee.clone();
                    ItemList::add_item(&mut reward_who, report_result_info.reporter.clone());
                    let _ = T::SlashAndReward::slash_and_reward(
                        slash_who.clone(),
                        report_result_info.committee_stake,
                        reward_who,
                    );

                    let _ = Self::change_reporter_stake(
                        &report_result_info.reporter,
                        report_result_info.reporter_stake,
                        false,
                    );

                    let _ = Self::change_committee_stake(
                        report_result_info.reward_committee.clone(),
                        report_result_info.committee_stake,
                        false,
                    );

                    let _ = Self::change_committee_stake(slash_who, report_result_info.committee_stake, true);
                },
                ReportResultType::NoConsensus => {
                    // only slash unruly_committee, no reward
                    let _ = T::SlashAndReward::slash_and_reward(
                        report_result_info.unruly_committee.clone(),
                        report_result_info.committee_stake,
                        vec![],
                    );

                    // TODO: ensure reporter & other committee is unreserved before
                    let _ = Self::change_committee_stake(
                        report_result_info.unruly_committee.clone(),
                        report_result_info.committee_stake,
                        true,
                    );
                },
                ReportResultType::ReportRefused => {
                    // slash reporter, slash committee
                    let _ = T::SlashAndReward::slash_and_reward(
                        vec![report_result_info.reporter.clone()],
                        report_result_info.reporter_stake,
                        report_result_info.reward_committee.clone(),
                    );

                    let mut slash_who = report_result_info.unruly_committee.clone();
                    for a_inconsistent in report_result_info.inconsistent_committee.clone() {
                        ItemList::add_item(&mut slash_who, a_inconsistent);
                    }

                    let _ = T::SlashAndReward::slash_and_reward(
                        slash_who.clone(),
                        report_result_info.committee_stake,
                        report_result_info.reward_committee.clone(),
                    );

                    let _ = Self::change_reporter_stake(
                        &report_result_info.reporter,
                        report_result_info.reporter_stake,
                        true,
                    );

                    let _ = Self::change_committee_stake(slash_who, report_result_info.committee_stake, true);
                    let _ = Self::change_committee_stake(
                        report_result_info.reward_committee.clone(),
                        report_result_info.committee_stake,
                        false,
                    );
                },
                ReportResultType::ReporterNotSubmitEncryptedInfo => {
                    // slash reporter, slash committee
                    let _ = T::SlashAndReward::slash_and_reward(
                        vec![report_result_info.reporter.clone()],
                        report_result_info.reporter_stake,
                        vec![],
                    );
                    let _ = T::SlashAndReward::slash_and_reward(
                        report_result_info.unruly_committee.clone(),
                        report_result_info.committee_stake,
                        vec![],
                    );

                    let _ = Self::change_reporter_stake(
                        &report_result_info.reporter,
                        report_result_info.reporter_stake,
                        true,
                    );
                    let _ = Self::change_committee_stake(
                        report_result_info.unruly_committee.clone(),
                        report_result_info.committee_stake,
                        true,
                    );
                    // TODO: ensure other committee has been unreserved
                },
            }

            ItemList::rm_item(&mut pending_unhandled_id, &slashed_report_id);
            report_result_info.slash_result = MCSlashResult::Executed;
            ReportResult::<T>::insert(slashed_report_id, report_result_info);
        }
        UnhandledReportResult::<T>::put(pending_unhandled_id);
        Ok(())
    }

    fn check_and_exec_pending_review() -> Result<(), ()> {
        let all_pending_review = <PendingSlashReview<T> as IterableStorageMap<ReportId, _>>::iter()
            .map(|(renter, _)| renter)
            .collect::<Vec<_>>();
        let now = <frame_system::Module<T>>::block_number();

        for a_pending_review in all_pending_review {
            let review_info = Self::pending_slash_review(a_pending_review);
            let report_result_info = Self::report_result(&a_pending_review);

            if review_info.expire_time < now {
                continue
            }

            let is_slashed_reporter = report_result_info.is_slashed_reporter(&review_info.applicant);
            let is_slashed_committee = report_result_info.is_slashed_committee(&review_info.applicant);
            let is_slashed_stash = report_result_info.is_slashed_stash(&review_info.applicant);

            if is_slashed_reporter {
                let _ = Self::change_reporter_stake(&review_info.applicant, review_info.staked_amount, true);
            } else if is_slashed_committee {
                let _ =
                    Self::change_committee_stake(vec![review_info.applicant.clone()], review_info.staked_amount, true);
            } else if is_slashed_stash {
                let _ = T::MTOps::mt_rm_stash_total_stake(review_info.applicant.clone(), review_info.staked_amount);
            }

            let _ = T::SlashAndReward::slash_and_reward(vec![review_info.applicant], review_info.staked_amount, vec![]);

            PendingSlashReview::<T>::remove(a_pending_review);
        }
        Ok(())
    }
}
