// 机器维护说明：
// 1. 报告人提出报告，系统生成报告ID
// 2. 报告人等待委员会抢单，在有委员会抢单之前，可以撤销报告
// 3. 委员会抢单
// 4. 报告人在委员会抢单半个小时内，提交加密信息给委员会，否则报告失败，将报告人的钱罚没，
// 奖励给提交了Hash的或者当前还没等到加密信息的委员会，
// 5. 委员会在抢单一个小时内，提交确认的Hash
// 6. 委员会没有在1个小时内提交Hash，则该委员会从已预订的列表中移除，添加到惩罚列表。
// 7. 委员会提交了Hash后，如果不满3个委员会，则继续抢单，重复上述流程。
// 8. 当三个委员会提交了Hash，或者时间达到第一个委员会抢单3小时，状态变为提交原始值阶段
// 9. 如果最后一个委员会不足1个小时，且没有提交Hash，将其移除，不奖励不惩罚
// 9. 等待到第四小时，或者直到所有提交了Hash的委员会都提交原始值，开始总结
// 10. 根据总结结果，
// 10.1. 对机器进行下线，对委员会进行奖励，对报告人进行奖励
// 10.2. 对报告人进行惩罚，对委员会进行奖励
// 奖励内容：
// 惩罚内容:
// 如果机器不存在，则...不会有这种情况的吧...
//
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

use codec::{Decode, Encode};
use frame_support::{
    pallet_prelude::*,
    traits::{Currency, LockableCurrency},
};
use frame_system::pallet_prelude::*;
use online_profile_machine::{DbcPrice, MTOps, ManageCommittee};
use sp_io::hashing::blake2_128;
use sp_runtime::{traits::SaturatedConversion, RuntimeDebug};
use sp_std::{prelude::*, str, vec::Vec};

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub type MachineId = Vec<u8>;
pub type ReportId = u64; // 提交的单据ID
pub type BoxPubkey = [u8; 32];
type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

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
    pub waiting_rechecked_report: Vec<ReportId>,
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
    /// 报告人pubkey
    pub reporter_boxpubkey: BoxPubkey,
    /// 报告人报告的Hash
    pub reporter_hash: [u8; 16],
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
    /// 硬件故障
    HardwareFault,
    /// 无法租用故障
    MachineUnrentable,
    /// 机器离线
    MachineOffline,
}

// 默认硬件故障
impl Default for MachineFaultType {
    fn default() -> Self {
        MachineFaultType::HardwareFault
    }
}

/// Summary after all committee submit raw info
enum ReportConfirmStatus<AccountId> {
    // Confirmed(Vec<AccountId>, Vec<u8>), // 带一个错误类型
    Confirmed(Vec<AccountId>, Vec<AccountId>, Vec<u8>),
    // Refuse(Vec<AccountId>),
    Refuse(Vec<AccountId>, Vec<AccountId>),
    NoConsensus,
}

/// 委员会抢到的报告的列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTCommitteeReportList {
    /// 委员会的报告
    pub booked_report: Vec<ReportId>,
    /// 已经提交了Hash信息的报告
    pub hashed_report: Vec<ReportId>,
    /// 已经提交了原始确认数据的报告
    pub confirmed_report: Vec<ReportId>,
    /// 已经成功上线的机器
    pub online_machine: Vec<MachineId>,
}

/// 委员会对报告的操作信息
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MTCommitteeOpsDetail<BlockNumber, Balance> {
    pub booked_time: BlockNumber,
    /// reporter 提交的加密后的信息
    pub encrypted_err_info: Option<Vec<u8>>,
    pub encrypted_time: BlockNumber,
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    pub confirm_raw: Vec<u8>,
    /// 委员会提交raw信息的时间
    pub confirm_time: BlockNumber,
    pub confirm_result: bool,
    pub staked_balance: Balance,
    pub order_status: MTOrderStatus,
}

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

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + generic_func::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type DbcPrice: DbcPrice<BalanceOf = BalanceOf<Self>>;
        type ManageCommittee: ManageCommittee<
            AccountId = Self::AccountId,
            BalanceOf = BalanceOf<Self>,
        >;
        type MTOps: MTOps<AccountId = Self::AccountId, MachineId = MachineId>;
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
            // Self::check_and_exec_slash();
        }
    }

    // 默认抢单委员会的个数
    #[pallet::type_value]
    pub fn CommitteeLimitDefault<T: Config>() -> u32 {
        3
    }

    // 最多多少个委员会能够抢单
    #[pallet::storage]
    #[pallet::getter(fn committee_limit)]
    pub(super) type CommitteeLimit<T: Config> =
        StorageValue<_, u32, ValueQuery, CommitteeLimitDefault<T>>;

    // 存储报告人在该模块中的总质押量
    #[pallet::storage]
    #[pallet::getter(fn user_total_stake)]
    pub(super) type UserTotalStake<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    // 查询报告人报告的机器
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
    #[pallet::getter(fn live_report)]
    pub(super) type LiveReport<T: Config> = StorageValue<_, MTLiveReportList, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_report_id)]
    pub(super) type NextReportId<T: Config> = StorageValue<_, ReportId, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// 用户报告机器有故障：无法租用或者硬件故障或者离线
        /// 报告无法租用提交Hash:机器ID+随机数+报告内容
        /// 报告硬件故障提交Hash:机器ID+随机数+报告内容+租用机器的Session信息
        /// 用户报告机器硬件故障
        #[pallet::weight(10000)]
        pub fn report_machine_fault(
            origin: OriginFor<T>,
            hash: [u8; 16],
            box_pubkey: BoxPubkey,
        ) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;

            Self::report_handler(
                reporter,
                MachineFaultType::HardwareFault,
                Some(hash),
                Some(box_pubkey),
                None,
            )
        }

        /// 用户报告机器无法租用
        #[pallet::weight(10000)]
        pub fn report_machine_unrentable(
            origin: OriginFor<T>,
            hash: [u8; 16],
            box_pubkey: BoxPubkey,
        ) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;

            Self::report_handler(
                reporter,
                MachineFaultType::MachineUnrentable,
                Some(hash),
                Some(box_pubkey),
                None,
            )
        }

        /// 用户报告机器掉线
        #[pallet::weight(10000)]
        pub fn report_machine_offline(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;

            Self::report_handler(
                reporter,
                MachineFaultType::MachineOffline,
                None,
                None,
                Some(machine_id),
            )
        }

        // 报告人可以在抢单之前取消该报告
        #[pallet::weight(10000)]
        pub fn reporter_cancle_report(
            origin: OriginFor<T>,
            report_id: ReportId,
        ) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;

            let report_info = Self::report_info(&report_id);
            ensure!(
                report_info.report_status == ReportStatus::Reported,
                Error::<T>::OrderNotAllowCancle
            );

            // 清理存储
            let mut live_report = Self::live_report();
            if let Ok(index) = live_report.bookable_report.binary_search(&report_id) {
                live_report.bookable_report.remove(index);
            }
            LiveReport::<T>::put(live_report);

            let mut reporter_report = Self::reporter_report(&reporter);
            if let Ok(index) = reporter_report.reported_id.binary_search(&report_id) {
                reporter_report.reported_id.remove(index);
            }
            ReporterReport::<T>::insert(&reporter, reporter_report);

            <T as pallet::Config>::ManageCommittee::change_stake(
                &reporter,
                report_info.reporter_stake,
                false,
            )
            .map_err(|_| Error::<T>::ReduceTotalStakeFailed)?;

            ReportInfo::<T>::remove(&report_id);

            Ok(().into())
        }

        // 委员会进行抢单
        // 状态变化：LiveReport的 bookable -> verifying_report
        // 报告状态变为Verifying
        // 订单状态变为WaitingEncrypt
        #[pallet::weight(10000)]
        pub fn book_fault_order(
            origin: OriginFor<T>,
            report_id: ReportId,
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            // 判断发起请求者是状态正常的委员会
            if !T::ManageCommittee::is_valid_committee(&committee) {
                return Err(Error::<T>::NotCommittee.into());
            }

            ensure!(<ReportInfo<T>>::contains_key(report_id), Error::<T>::OrderNotAllowBook);

            // 检查订单是否可预订状态
            let mut report_info = Self::report_info(report_id);
            let mut ops_detail = Self::committee_ops(&committee, &report_id);
            let mut live_report = Self::live_report();

            // 检查订单是否可以抢定
            ensure!(
                report_info.report_status == ReportStatus::Reported
                    || report_info.report_status == ReportStatus::WaitingBook,
                Error::<T>::OrderNotAllowBook
            );

            // 当有三个委员会已经抢单时，禁止抢单
            if report_info.booked_committee.len() == 3 {
                return Err(Error::<T>::OrderNotAllowBook.into());
            }

            // 记录预订订单的委员会
            if let Err(index) = report_info.booked_committee.binary_search(&committee) {
                report_info.booked_committee.insert(index, committee.clone());
            } else {
                return Err(Error::<T>::AlreadyBooked.into());
            }

            // 支付手续费或押金
            match report_info.machine_fault_type {
                MachineFaultType::HardwareFault | MachineFaultType::MachineUnrentable => {
                    // 此两种情况，需要质押100RMB等值DBC
                    let committee_order_stake = T::ManageCommittee::stake_per_order()
                        .ok_or(Error::<T>::GetStakeAmountFailed)?;

                    <T as pallet::Config>::ManageCommittee::change_stake(
                        &committee,
                        committee_order_stake,
                        true,
                    )
                    .map_err(|_| Error::<T>::StakeFailed)?;
                    ops_detail.staked_balance = committee_order_stake;

                    // 改变report状态为正在验证中，此时禁止其他委员会预订
                    report_info.report_status = ReportStatus::Verifying;

                    // 记录第一个预订订单的时间, 3个小时(360个块)之后开始提交原始值
                    if report_info.booked_committee.len() == 1 {
                        report_info.first_book_time = now;
                        report_info.confirm_start = now + 360u32.saturated_into::<T::BlockNumber>();
                    }

                    // 从bookable_report移动到verifying_report
                    if let Ok(index) = live_report.bookable_report.binary_search(&report_id) {
                        live_report.bookable_report.remove(index);
                    }
                    if let Err(index) = live_report.verifying_report.binary_search(&report_id) {
                        live_report.verifying_report.insert(index, report_id);
                    }
                    LiveReport::<T>::put(live_report);
                }
                MachineFaultType::MachineOffline => {
                    // 付10个DBC的手续费
                    <generic_func::Module<T>>::pay_fixed_tx_fee(committee.clone())
                        .map_err(|_| Error::<T>::PayTxFeeFailed)?;

                    // WaitingBook状态允许其他委员会继续抢单
                    report_info.report_status = ReportStatus::WaitingBook;

                    // 记录第一个预订订单的时间, 5分钟(10个块)之后开始提交原始值
                    if report_info.booked_committee.len() == 1 {
                        report_info.first_book_time = now;
                        report_info.confirm_start = now + 10u32.saturated_into::<T::BlockNumber>();
                    }
                }
            }

            // 记录当前哪个委员会正在验证，方便状态控制
            report_info.verifying_committee = Some(committee.clone());

            // 添加到委员会自己的存储中
            let mut committee_order = Self::committee_order(&committee);
            if let Err(index) = committee_order.booked_report.binary_search(&report_id) {
                committee_order.booked_report.insert(index, report_id);
            }
            CommitteeOrder::<T>::insert(&committee, committee_order);

            // 添加委员会对于机器的操作记录
            ops_detail.booked_time = now;
            ops_detail.order_status = MTOrderStatus::WaitingEncrypt;
            CommitteeOps::<T>::insert(&committee, &report_id, ops_detail);

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

            // 检查该reporter拥有这个订单
            let reporter_report = Self::reporter_report(&reporter);
            reporter_report
                .reported_id
                .binary_search(&report_id)
                .map_err(|_| Error::<T>::NotOrderReporter)?;

            // 该orde处于验证中, 且还没有提交过加密信息
            let mut report_info = Self::report_info(&report_id);
            if let MachineFaultType::MachineOffline = report_info.machine_fault_type {
                return Err(Error::<T>::NotNeedEncryptedInfo.into());
            }

            let mut committee_ops = Self::committee_ops(&to_committee, &report_id);
            ensure!(
                report_info.report_status == ReportStatus::Verifying,
                Error::<T>::OrderStatusNotFeat
            );
            ensure!(
                committee_ops.order_status == MTOrderStatus::WaitingEncrypt,
                Error::<T>::OrderStatusNotFeat
            );
            // 检查该委员会为预订了该订单的委员会
            report_info
                .booked_committee
                .binary_search(&to_committee)
                .map_err(|_| Error::<T>::NotOrderCommittee)?;

            // report_info中插入已经收到了加密信息的委员会
            if let Err(index) =
                report_info.get_encrypted_info_committee.binary_search(&to_committee)
            {
                report_info.get_encrypted_info_committee.insert(index, to_committee.clone());
            }

            committee_ops.encrypted_err_info = Some(encrypted_err_info);
            committee_ops.encrypted_time = now;
            committee_ops.order_status = MTOrderStatus::Verifying;

            CommitteeOps::<T>::insert(&to_committee, &report_id, committee_ops);
            ReportInfo::<T>::insert(report_id, report_info);

            Ok(().into())
        }

        // 委员会提交验证之后的Hash
        // 用户必须在自己的Order状态为Verifying时提交Hash
        #[pallet::weight(10000)]
        pub fn submit_confirm_hash(
            origin: OriginFor<T>,
            report_id: ReportId,
            hash: [u8; 16],
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let committee_limit = Self::committee_limit();

            // 判断是否为委员会其列表是否有该report_id
            let mut committee_order = Self::committee_order(&committee);
            committee_order
                .booked_report
                .binary_search(&report_id)
                .map_err(|_| Error::<T>::NotInBookedList)?;

            let mut committee_ops = Self::committee_ops(&committee, &report_id);
            // 判断该委员会的状态是验证中
            ensure!(
                committee_ops.order_status == MTOrderStatus::Verifying,
                Error::<T>::OrderStatusNotFeat
            );

            // 判断该report_id是否可以提交信息
            let mut report_info = Self::report_info(&report_id);
            ensure!(
                report_info.report_status == ReportStatus::Verifying,
                Error::<T>::OrderStatusNotFeat
            );

            // 添加到report的已提交Hash的委员会列表
            if let Err(index) = report_info.hashed_committee.binary_search(&committee) {
                report_info.hashed_committee.insert(index, committee.clone());
            }

            let mut live_report = Self::live_report();

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

            if let ReportStatus::SubmittingRaw = report_info.report_status {
                LiveReport::<T>::put(live_report);
            }
            CommitteeOps::<T>::insert(&committee, &report_id, committee_ops);
            CommitteeOrder::<T>::insert(&committee, committee_order);
            ReportInfo::<T>::insert(&report_id, report_info);
            Ok(().into())
        }

        // 订单状态必须是等待SubmittingRaw
        #[pallet::weight(10000)]
        pub fn submit_confirm_raw(
            origin: OriginFor<T>,
            report_id: ReportId,
            machine_id: MachineId,
            reporter_rand_str: Vec<u8>,
            committee_rand_str: Vec<u8>,
            err_reason: Vec<u8>,
            support_report: bool,
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut report_info = Self::report_info(report_id);
            ensure!(
                report_info.report_status == ReportStatus::SubmittingRaw,
                Error::<T>::OrderStatusNotFeat
            );

            if let MachineFaultType::MachineOffline = report_info.machine_fault_type {
                return Err(Error::<T>::OrderStatusNotFeat.into());
            }

            if let MachineFaultType::MachineOffline = report_info.machine_fault_type {
                return Err(Error::<T>::OrderStatusNotFeat.into());
            }

            // let fault_info_hash = match report_info.machine_fault_type {
            //     MachineFaultType::HardwareFault(hash, _) => hash,
            //     MachineFaultType::MachineUnrentable(hash, _) => hash,
            //     MachineFaultType::MachineOffline(_) => {
            //         return Err(Error::<T>::OrderStatusNotFeat.into())
            //     }
            // };

            // 检查是否提交了该订单的hash
            report_info
                .hashed_committee
                .binary_search(&committee)
                .map_err(|_| Error::<T>::NotProperCommittee)?;

            // 添加到Report的已提交Raw的列表
            if let Ok(index) = report_info.confirmed_committee.binary_search(&committee) {
                report_info.confirmed_committee.insert(index, committee.clone());
            }

            let mut committee_ops = Self::committee_ops(&committee, &report_id);

            // 检查是否与报告人提交的Hash一致
            let mut reporter_info_raw = Vec::new();
            reporter_info_raw.extend(machine_id.clone());
            reporter_info_raw.extend(reporter_rand_str.clone());
            reporter_info_raw.extend(err_reason.clone());
            let reporter_report_hash = Self::get_hash(&reporter_info_raw);
            if reporter_report_hash != report_info.reporter_hash {
                return Err(Error::<T>::NotEqualReporterSubmit.into());
            }

            // 检查委员会提交是否与第一次Hash一致
            let mut committee_report_raw = Vec::new();
            committee_report_raw.extend(machine_id.clone());
            committee_report_raw.extend(reporter_rand_str);
            committee_report_raw.extend(committee_rand_str);
            let is_support: Vec<u8> = if support_report { "1".into() } else { "0".into() };
            committee_report_raw.extend(is_support);
            committee_report_raw.extend(err_reason.clone());
            let committee_report_hash = Self::get_hash(&committee_report_raw);
            if committee_report_hash != committee_ops.confirm_hash {
                return Err(Error::<T>::NotEqualCommitteeSubmit.into());
            }

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

            // 判断是否订阅的用户全部提交了Raw，如果是则进入下一阶段
            if report_info.hashed_committee.len() == report_info.confirmed_committee.len() {
                Self::summary_report(report_id);
            }

            CommitteeOps::<T>::insert(&committee, &report_id, committee_ops);
            ReportInfo::<T>::insert(&report_id, report_info);

            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ReportMachineFault(T::AccountId, MachineFaultType),
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
        OrderNotAllowCancle,
        OrderNotAllowBook,
        NotProperCommittee,
        NotEqualReporterSubmit,
        NotEqualCommitteeSubmit,
        ReduceTotalStakeFailed,
        PayTxFeeFailed,
        NotNeedEncryptedInfo,
    }
}

impl<T: Config> Pallet<T> {
    pub fn report_handler(
        reporter: T::AccountId,
        machine_fault_type: MachineFaultType,
        hash: Option<[u8; 16]>,
        box_pubkey: Option<BoxPubkey>,
        machine_id: Option<MachineId>,
    ) -> DispatchResultWithPostInfo {
        let report_time = <frame_system::Module<T>>::block_number();
        let report_id = Self::get_new_report_id();

        let stake_need = <T as pallet::Config>::ManageCommittee::stake_per_order()
            .ok_or(Error::<T>::GetStakeAmountFailed)?;
        <T as pallet::Config>::ManageCommittee::change_stake(&reporter, stake_need, true)
            .map_err(|_| Error::<T>::StakeFailed)?;

        // 被报告的机器存储起来，委员会进行抢单
        let mut live_report = Self::live_report();
        if let Err(index) = live_report.bookable_report.binary_search(&report_id) {
            live_report.bookable_report.insert(index, report_id);
        }
        LiveReport::<T>::put(live_report);

        match machine_fault_type.clone() {
            // 当是前面两种情况时，记录下Hash和box_pubkey
            MachineFaultType::HardwareFault | MachineFaultType::MachineUnrentable => {
                ReportInfo::<T>::insert(
                    &report_id,
                    MTReportInfoDetail {
                        reporter: reporter.clone(),
                        report_time,
                        reporter_stake: stake_need,
                        machine_fault_type: machine_fault_type.clone(),
                        report_status: ReportStatus::Reported,
                        reporter_boxpubkey: box_pubkey.unwrap(),
                        reporter_hash: hash.unwrap(),
                        ..Default::default()
                    },
                );
            }
            // 当是offline时，记录下MachineId，还需要10个DBC作为手续费
            MachineFaultType::MachineOffline => {
                <generic_func::Module<T>>::pay_fixed_tx_fee(reporter.clone())
                    .map_err(|_| Error::<T>::PayTxFeeFailed)?;

                ReportInfo::<T>::insert(
                    &report_id,
                    MTReportInfoDetail {
                        reporter: reporter.clone(),
                        report_time,
                        reporter_stake: stake_need,
                        machine_fault_type: machine_fault_type.clone(),
                        machine_id: machine_id.unwrap(),
                        report_status: ReportStatus::Reported,
                        ..Default::default()
                    },
                );
            }
        }

        // 记录到报告人的存储中
        let mut reporter_report = Self::reporter_report(&reporter);
        if let Err(index) = reporter_report.reported_id.binary_search(&report_id) {
            reporter_report.reported_id.insert(index, report_id);
        }
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

        return report_id;
    }

    fn get_hash(raw_str: &Vec<u8>) -> [u8; 16] {
        return blake2_128(raw_str);
    }

    // 处理用户没有发送加密信息的订单
    // 对用户进行惩罚，对委员会进行奖励
    fn refund_committee_clean_report(report_id: ReportId) {
        let report_info = Self::report_info(report_id);

        // 清理每个委员会存储
        for a_committee in report_info.booked_committee {
            let committee_ops = Self::committee_ops(&a_committee, &report_id);

            if <T as pallet::Config>::ManageCommittee::change_stake(
                &a_committee,
                committee_ops.staked_balance,
                false,
            )
            .is_err()
            {
                debug::error!("Reduce committee stake failed");
            };

            CommitteeOps::<T>::remove(&a_committee, &report_id);

            Self::clean_from_committee_order(&a_committee, &report_id);
        }

        // 清理该报告
        Self::clean_from_live_report(&report_id);
        ReportInfo::<T>::remove(&report_id);
    }

    // 从委员会的订单列表中删除
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

    // 从live_report中移除一个订单
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
        if let Ok(index) = live_report.waiting_rechecked_report.binary_search(report_id) {
            live_report.waiting_rechecked_report.remove(index);
        }
        LiveReport::<T>::put(live_report);
    }

    // Summary committee's handle result
    fn summary_report(report_id: ReportId) -> ReportConfirmStatus<T::AccountId> {
        let report_info = Self::report_info(&report_id);
        // 如果没有委员会提交Raw信息，则无共识
        if report_info.confirmed_committee.len() == 0 {
            return ReportConfirmStatus::NoConsensus;
        }

        if report_info.support_committee.len() >= report_info.against_committee.len() {
            return ReportConfirmStatus::Confirmed(
                report_info.support_committee,
                report_info.against_committee,
                report_info.err_info.clone(),
            );
        }

        return ReportConfirmStatus::Refuse(
            report_info.support_committee,
            report_info.against_committee,
        );
    }

    fn heart_beat() {
        let now = <frame_system::Module<T>>::block_number();
        let mut live_report = Self::live_report();
        let verifying_report = live_report.verifying_report.clone();
        let submitting_raw_report = live_report.waiting_raw_report.clone();

        let half_hour = 60u64.saturated_into::<T::BlockNumber>();
        let one_hour = 120u64.saturated_into::<T::BlockNumber>();
        let three_hour = 360u64.saturated_into::<T::BlockNumber>();
        let four_hour = 480u64.saturated_into::<T::BlockNumber>();

        for a_report in verifying_report {
            let mut report_info = Self::report_info(&a_report);

            // 不足3小时
            if now - report_info.report_time <= three_hour {
                if let ReportStatus::WaitingBook = report_info.report_status {
                    continue;
                }

                let verifying_committee = report_info.verifying_committee.as_ref().unwrap().clone();
                let committee_ops = Self::committee_ops(&verifying_committee, &a_report);

                // 报告人没有提交给原始信息，则惩罚报告人到国库，不进行奖励
                if committee_ops.encrypted_err_info.is_none()
                    && now - committee_ops.booked_time >= half_hour
                {
                    <T as pallet::Config>::ManageCommittee::add_slash(
                        report_info.reporter,
                        report_info.reporter_stake,
                        Vec::new(),
                    );

                    Self::refund_committee_clean_report(a_report);

                    continue;
                }

                // 不足3小时，且委员会没有提交Hash，删除该委员会，并惩罚
                if now - committee_ops.booked_time >= one_hour {
                    report_info.verifying_committee = None;
                    report_info.booked_committee.remove(report_info.booked_committee.len() - 1);
                    report_info.report_status = ReportStatus::WaitingBook;

                    MTLiveReportList::rm_report_id(&mut live_report.verifying_report, a_report);
                    MTLiveReportList::add_report_id(&mut live_report.bookable_report, a_report);

                    // slash committee
                    <T as pallet::Config>::ManageCommittee::add_slash(
                        verifying_committee.clone(),
                        committee_ops.staked_balance,
                        Vec::new(),
                    );

                    ReportInfo::<T>::insert(a_report, report_info);
                    CommitteeOps::<T>::remove(&verifying_committee, &a_report);

                    continue;
                }
            }

            // 已经到3个小时
            if now - report_info.report_time >= three_hour {
                if let ReportStatus::WaitingBook = report_info.report_status {
                    report_info.report_status = ReportStatus::SubmittingRaw;

                    MTLiveReportList::rm_report_id(&mut live_report.verifying_report, a_report);
                    MTLiveReportList::add_report_id(&mut live_report.waiting_raw_report, a_report);

                    ReportInfo::<T>::insert(a_report, report_info);
                    continue;
                }

                // 但是最后一个委员会订阅时间小于1个小时
                let verifying_committee = report_info.verifying_committee.unwrap().clone();
                let committee_ops = Self::committee_ops(&verifying_committee, &a_report);

                if now - committee_ops.booked_time < one_hour {
                    // 将最后一个委员会移除，并不惩罚
                    report_info.verifying_committee = None;
                    report_info.booked_committee.remove(report_info.booked_committee.len() - 1);
                    report_info.report_status = ReportStatus::SubmittingRaw;

                    MTLiveReportList::rm_report_id(&mut live_report.verifying_report, a_report);
                    MTLiveReportList::add_report_id(&mut live_report.waiting_raw_report, a_report);

                    ReportInfo::<T>::insert(a_report, report_info);
                    continue;
                }
            }
        }

        // 正在提交原始值的
        for a_report in submitting_raw_report {
            let mut report_info = Self::report_info(&a_report);
            // 未全部提交了原始信息且未达到了四个小时
            if now - report_info.report_time < four_hour
                && report_info.hashed_committee.len() != report_info.confirmed_committee.len()
            {
                continue;
            }

            match Self::summary_report(a_report) {
                ReportConfirmStatus::Confirmed(
                    support_committees,
                    against_committee,
                    _err_info,
                ) => {
                    for a_committee in against_committee.clone() {
                        let committee_ops = Self::committee_ops(&a_committee, a_report);
                        T::ManageCommittee::add_slash(
                            report_info.reporter.clone(),
                            committee_ops.staked_balance,
                            Vec::new(),
                        );
                    }

                    for a_committee in support_committees.clone() {
                        let committee_ops = Self::committee_ops(&a_committee, a_report);
                        if let Err(e) = T::ManageCommittee::change_stake(
                            &a_committee,
                            committee_ops.staked_balance,
                            false,
                        ) {
                            debug::error!("Change stake of {:?} failed: {:?}", &a_committee, e);
                        }
                    }

                    T::MTOps::mt_machine_offline(report_info.machine_id.clone());
                }
                ReportConfirmStatus::Refuse(support_committee, against_committee) => {
                    for a_committee in support_committee {
                        let committee_ops = Self::committee_ops(a_committee.clone(), a_report);
                        T::ManageCommittee::add_slash(
                            a_committee,
                            committee_ops.staked_balance,
                            against_committee.clone(),
                        );
                    }

                    for a_committee in against_committee.clone() {
                        let committee_ops = Self::committee_ops(&a_committee, a_report);
                        if let Err(e) = T::ManageCommittee::change_stake(
                            &a_committee,
                            committee_ops.staked_balance,
                            false,
                        ) {
                            debug::error!("Change stake of {:?} failed: {:?}", &a_committee, e);
                        };
                    }

                    T::ManageCommittee::add_slash(
                        report_info.reporter.clone(),
                        report_info.reporter_stake,
                        against_committee.clone(),
                    );
                }
                // No consensus, will clean record & as new report to handle
                // In this case, no raw info is submitted, so committee record should be None
                ReportConfirmStatus::NoConsensus => {
                    report_info.report_status = ReportStatus::Reported;
                    MTLiveReportList::add_report_id(&mut live_report.bookable_report, a_report);
                    MTLiveReportList::rm_report_id(&mut live_report.verifying_report, a_report);
                    MTLiveReportList::rm_report_id(&mut live_report.waiting_raw_report, a_report);
                }
            }
            ReportInfo::<T>::insert(a_report, report_info);
        }

        LiveReport::<T>::put(live_report);
    }
}
