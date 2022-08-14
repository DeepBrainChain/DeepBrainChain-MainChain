#![cfg_attr(not(feature = "std"), no_std)]

mod slash;
mod types;
mod utils;

#[cfg(test)]
mod mock;
#[cfg(test)]
#[allow(non_upper_case_globals)]
mod tests;

use codec::alloc::string::ToString;
use frame_support::{
    pallet_prelude::*,
    traits::{Currency, OnUnbalanced, ReservableCurrency},
};
use frame_system::pallet_prelude::*;
use generic_func::{ItemList, MachineId};
use online_profile_machine::{GNOps, MTOps, ManageCommittee};
use sp_runtime::traits::{Saturating, Zero};
use sp_std::{str, vec, vec::Vec};
use rent_machine::RentOrderId;

pub use pallet::*;
use types::*;

type BalanceOf<T> = <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + online_profile::Config + generic_func::Config + rent_machine::Config
    {
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
            // TODO: 更改为用户手动操作时触发
            let _ = Self::summary_fault_hook();
            let _ = Self::summary_inaccessible_hook();
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

    #[pallet::storage]
    #[pallet::getter(fn reporter_stake_params)]
    pub(super) type ReporterStakeParams<T: Config> = StorageValue<_, ReporterStakeParamsInfo<BalanceOf<T>>>;

    #[pallet::storage]
    #[pallet::getter(fn next_report_id)]
    pub(super) type NextReportId<T: Config> = StorageValue<_, ReportId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn reporter_stake)]
    pub(super) type ReporterStake<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, ReporterStakeInfo<BalanceOf<T>>, ValueQuery>;

    /// Report record for reporter
    #[pallet::storage]
    #[pallet::getter(fn reporter_report)]
    pub(super) type ReporterReport<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, ReporterReportList, ValueQuery>;

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

    /// 系统中还未完成的订单
    #[pallet::storage]
    #[pallet::getter(fn live_report)]
    pub(super) type LiveReport<T: Config> = StorageValue<_, MTLiveReportList, ValueQuery>;

    /// 系统中还未完成的订单
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
    pub(super) type UnhandledReportResult<T: Config> =
        StorageMap<_, Blake2_128Concat, T::BlockNumber, Vec<ReportId>, ValueQuery>;

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

        /// 用户报告机器硬件故障
        #[pallet::weight(10000)]
        pub fn report_machine_fault(
            origin: OriginFor<T>,
            report_reason: MachineFaultType,
        ) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;

            let mut live_report = Self::live_report();
            let mut reporter_report = Self::reporter_report(&reporter);
            let stake_params = Self::reporter_stake_params().ok_or(Error::<T>::GetStakeAmountFailed)?;

            // 支付
            if let MachineFaultType::RentedInaccessible(machine_id, rent_order_id) = report_reason.clone() {
                // 检查是否是机器租用者
                let rent_order = <rent_machine::Module<T>>::rent_order(&rent_order_id);
                ensure!(rent_order.renter == reporter, Error::<T>::NotMachineRenter);
                <generic_func::Module<T>>::pay_fixed_tx_fee(reporter.clone())
                    .map_err(|_| Error::<T>::PayTxFeeFailed)?;
            }
            Self::pay_stake_when_report(reporter.clone(), &stake_params)?;

            let _ = Self::do_report_machine_fault(
                reporter.clone(),
                report_reason,
                None,
                &mut live_report,
                &mut reporter_report,
            );

            LiveReport::<T>::put(live_report);
            ReporterReport::<T>::insert(&reporter, reporter_report);
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn reporter_add_stake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            Self::change_reporter_stake(reporter, amount, true)
        }

        #[pallet::weight(10000)]
        pub fn reporter_reduce_stake(origin: OriginFor<T>, amount: BalanceOf<T>) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            Self::change_reporter_stake(reporter, amount, false)
        }

        // 报告人可以在抢单之前取消该报告
        #[pallet::weight(10000)]
        pub fn reporter_cancel_report(origin: OriginFor<T>, report_id: ReportId) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            let report_info = Self::report_info(&report_id);

            ensure!(report_info.reporter == reporter, Error::<T>::NotReporter);
            ensure!(report_info.report_status == ReportStatus::Reported, Error::<T>::OrderNotAllowCancel);
            ensure!(
                Self::change_reporter_stake_on_report_close(&reporter, report_info.reporter_stake, false).is_ok(),
                Error::<T>::ReduceTotalStakeFailed
            );

            Self::do_reporter_cancel_report(&reporter, report_id);
            Self::deposit_event(Event::ReportCanceld(reporter, report_id, report_info.machine_fault_type));
            Ok(().into())
        }

        /// 委员会进行抢单
        #[pallet::weight(10000)]
        pub fn committee_book_report(origin: OriginFor<T>, report_id: ReportId) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;

            ensure!(<T as pallet::Config>::ManageCommittee::is_valid_committee(&committee), Error::<T>::NotCommittee);

            let mut report_info = Self::report_info(report_id);
            // 检查订单是否可以抢定
            ensure!(report_info.report_time != Zero::zero(), Error::<T>::OrderNotAllowBook);
            ensure!(
                report_info.report_status == ReportStatus::Reported
                    || report_info.report_status == ReportStatus::WaitingBook,
                Error::<T>::OrderNotAllowBook
            );
            ensure!(report_info.booked_committee.len() < 3, Error::<T>::OrderNotAllowBook);
            ensure!(report_info.booked_committee.binary_search(&committee).is_err(), Error::<T>::AlreadyBooked);

            let order_stake =
                <T as pallet::Config>::ManageCommittee::stake_per_order().ok_or(Error::<T>::GetStakeAmountFailed)?;

            // 支付手续费或押金: 10 DBC | 1000 DBC
            if let MachineFaultType::RentedInaccessible(..) = report_info.machine_fault_type {
                <generic_func::Module<T>>::pay_fixed_tx_fee(committee.clone())
                    .map_err(|_| Error::<T>::PayTxFeeFailed)?;
            } else {
                <T as pallet::Config>::ManageCommittee::change_used_stake(committee.clone(), order_stake, true)
                    .map_err(|_| Error::<T>::StakeFailed)?;
            }

            Self::do_book_reports(committee.clone(), report_id, &mut report_info, order_stake);
            Self::deposit_event(Event::CommitteeBookReport(committee, report_id));
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
            let mut committee_ops = Self::committee_ops(&to_committee, &report_id);

            if let MachineFaultType::RentedInaccessible(..) = report_info.machine_fault_type {
                return Err(Error::<T>::NotNeedEncryptedInfo.into());
            }
            ensure!(report_info.reporter == reporter, Error::<T>::NotOrderReporter);
            ensure!(report_info.report_status == ReportStatus::Verifying, Error::<T>::OrderStatusNotFeat);
            ensure!(report_info.booked_committee.binary_search(&to_committee).is_ok(), Error::<T>::NotOrderCommittee);

            ensure!(committee_ops.order_status == MTOrderStatus::WaitingEncrypt, Error::<T>::OrderStatusNotFeat);

            // report_info中插入已经收到了加密信息的委员会
            ItemList::add_item(&mut report_info.get_encrypted_info_committee, to_committee.clone());
            ReportInfo::<T>::insert(&report_id, report_info);

            committee_ops.add_encry_info(encrypted_err_info, now);
            CommitteeOps::<T>::insert(&to_committee, &report_id, committee_ops);

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

            let mut committee_order = Self::committee_order(&committee);
            let mut committee_ops = Self::committee_ops(&committee, &report_id);
            let mut report_info = Self::report_info(&report_id);
            let mut live_report = Self::live_report();

            ensure!(committee_order.booked_report.binary_search(&report_id).is_ok(), Error::<T>::NotInBookedList);
            ensure!(committee_ops.order_status == MTOrderStatus::Verifying, Error::<T>::OrderStatusNotFeat);

            let is_inaccess = matches!(report_info.machine_fault_type, MachineFaultType::RentedInaccessible(..));

            if is_inaccess {
                ensure!(
                    report_info.report_status == ReportStatus::WaitingBook
                        || report_info.report_status == ReportStatus::Verifying,
                    Error::<T>::OrderStatusNotFeat
                );
            } else {
                ensure!(report_info.report_status == ReportStatus::Verifying, Error::<T>::OrderStatusNotFeat);
            }
            // 判断Hash是否被提交过
            for a_committee in &report_info.hashed_committee {
                let committee_ops = Self::committee_ops(&a_committee, report_id);
                if committee_ops.confirm_hash == hash {
                    return Err(Error::<T>::DuplicateHash.into());
                }
            }

            // 修改report_info
            report_info.add_hash(committee.clone(), committee_limit, is_inaccess);
            // 修改live_report
            if report_info.hashed_committee.len() == committee_limit as usize {
                // 全都提交了hash后，进入提交raw的阶段
                ItemList::rm_item(&mut live_report.verifying_report, &report_id);
                ItemList::add_item(&mut live_report.waiting_raw_report, report_id);
            } else if !is_inaccess {
                // 否则，是普通错误时，继续允许预订
                ItemList::rm_item(&mut live_report.verifying_report, &report_id);
                ItemList::add_item(&mut live_report.bookable_report, report_id);
            }
            // 修改committeeOps存储/状态
            committee_ops.add_hash(hash, now);
            // 修改committee_order 预订 -> Hash
            committee_order.add_hash(report_id);

            ReportInfo::<T>::insert(&report_id, report_info);
            LiveReport::<T>::put(live_report);
            CommitteeOps::<T>::insert(&committee, &report_id, committee_ops);
            CommitteeOrder::<T>::insert(&committee, committee_order);

            Self::deposit_event(Event::HashSubmited(report_id, committee));
            Ok(().into())
        }

        /// 订单状态必须是等待SubmittingRaw: 除了offline之外的所有错误类型
        #[pallet::weight(10000)]
        pub fn committee_submit_verify_raw(
            origin: OriginFor<T>,
            report_id: ReportId,
            machine_id: MachineId,
            rent_order_id: RentOrderId,
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

            // 获取链上已经记录的Hash
            let reporter_hash =
                report_info.machine_fault_type.clone().get_hash().ok_or(Error::<T>::OrderStatusNotFeat)?;
            // 检查是否提交了该订单的hash
            ensure!(report_info.hashed_committee.binary_search(&committee).is_ok(), Error::<T>::NotProperCommittee);

            // 检查是否与报告人提交的Hash一致
            let reporter_report_hash =
                Self::get_hash(vec![machine_id.clone(), reporter_rand_str.clone(), err_reason.clone()]);
            ensure!(reporter_report_hash == reporter_hash, Error::<T>::NotEqualReporterSubmit);

            let mut committee_ops = Self::committee_ops(&committee, &report_id);
            let mut committee_order = Self::committee_order(&committee);

            // 检查委员会提交是否与第一次Hash一致
            let is_support: Vec<u8> = if support_report { "1".into() } else { "0".into() };
            let committee_report_hash = Self::get_hash(vec![
                machine_id.clone(),
                rent_order_id.to_string().into(),
                reporter_rand_str,
                committee_rand_str,
                is_support,
                err_reason.clone(),
            ]);
            ensure!(committee_report_hash == committee_ops.confirm_hash, Error::<T>::NotEqualCommitteeSubmit);

            // 更改report_info，添加提交Raw的记录
            report_info.add_raw(committee.clone(), support_report, Some(machine_id), err_reason);
            // 记录committee_ops，添加提交Raw记录
            committee_ops.add_raw(now, support_report, extra_err_info);
            // 记录committee_order
            committee_order.add_raw(report_id);

            CommitteeOrder::<T>::insert(&committee, committee_order);
            CommitteeOps::<T>::insert(&committee, &report_id, committee_ops);
            ReportInfo::<T>::insert(&report_id, report_info);

            Self::deposit_event(Event::RawInfoSubmited(report_id, committee));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn committee_submit_inaccessible_raw(
            origin: OriginFor<T>,
            report_id: ReportId,
            committee_rand_str: Vec<u8>,
            is_support: bool,
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut report_info = Self::report_info(report_id);
            let mut committee_order = Self::committee_order(&committee);
            let mut committee_ops = Self::committee_ops(&committee, &report_id);

            ensure!(report_info.report_status == ReportStatus::SubmittingRaw, Error::<T>::OrderStatusNotFeat);
            match report_info.machine_fault_type {
                MachineFaultType::RentedInaccessible(..) => {},
                _ => return Err(Error::<T>::OrderStatusNotFeat.into()),
            }
            // 检查是否提交了该订单的hash
            ensure!(report_info.hashed_committee.binary_search(&committee).is_ok(), Error::<T>::NotProperCommittee);

            // 检查Hash是否一致
            let is_support_u8: Vec<u8> = if is_support { "1".into() } else { "0".into() };
            ensure!(
                Self::get_hash(vec![report_id.to_string().into(), committee_rand_str, is_support_u8])
                    == committee_ops.confirm_hash,
                Error::<T>::NotEqualCommitteeSubmit
            );

            // 记录到report_info中
            report_info.add_raw(committee.clone(), is_support, None, vec![]);
            // 记录到committee_ops
            committee_ops.add_raw(now, is_support, vec![]);
            // 记录到committee_order
            committee_order.add_raw(report_id);

            ReportInfo::<T>::insert(&report_id, report_info);
            CommitteeOps::<T>::insert(&committee, &report_id, committee_ops);
            CommitteeOrder::<T>::insert(&committee, committee_order);

            Self::deposit_event(Event::RawInfoSubmited(report_id, committee));
            Ok(().into())
        }

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
            let report_result_info = Self::report_result(report_result_id);

            // 判断申请人角色
            let stake_per_report = reporter_stake_params.stake_per_report;
            let is_slashed_reporter = report_result_info.is_slashed_reporter(&applicant);
            let is_slashed_committee = report_result_info.is_slashed_committee(&applicant);
            let is_slashed_stash = report_result_info.is_slashed_stash(&applicant);

            ensure!(!PendingSlashReview::<T>::contains_key(report_result_id), Error::<T>::AlreadyApplied);
            ensure!(is_slashed_reporter || is_slashed_committee || is_slashed_stash, Error::<T>::NotSlashed);
            ensure!(now < report_result_info.slash_exec_time, Error::<T>::TimeNotAllowed);
            ensure!(<T as Config>::Currency::can_reserve(&applicant, stake_per_report), Error::<T>::BalanceNotEnough);

            // Add stake when apply for review
            // NOTE: here, should add total stake and **also add used stake**
            if is_slashed_reporter {
                Self::change_reporter_stake(applicant.clone(), stake_per_report, true)?;
                Self::pay_stake_when_report(applicant.clone(), &reporter_stake_params)?;
            } else if is_slashed_committee {
                // Change committee stake
                <T as pallet::Config>::ManageCommittee::change_total_stake(
                    applicant.clone(),
                    stake_per_report,
                    true,
                    true,
                )
                .map_err(|_| Error::<T>::BalanceNotEnough)?;

                <T as pallet::Config>::ManageCommittee::change_used_stake(applicant.clone(), stake_per_report, true)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;
            } else if is_slashed_stash {
                T::MTOps::mt_change_staked_balance(applicant.clone(), stake_per_report, true)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;
            }

            PendingSlashReview::<T>::insert(
                report_result_id,
                MTPendingSlashReviewInfo {
                    applicant,
                    staked_amount: stake_per_report,
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
            <T as pallet::Config>::CancelSlashOrigin::ensure_origin(origin)?;
            ensure!(ReportResult::<T>::contains_key(slashed_report_id), Error::<T>::SlashIdNotExist);
            ensure!(PendingSlashReview::<T>::contains_key(slashed_report_id), Error::<T>::NotPendingReviewSlash);

            let now = <frame_system::Module<T>>::block_number();
            let mut report_result = Self::report_result(slashed_report_id);
            let slash_review_info = Self::pending_slash_review(slashed_report_id);
            let (applicant, staked) = (slash_review_info.applicant, slash_review_info.staked_amount);

            ensure!(slash_review_info.expire_time > now, Error::<T>::ExpiredApply);

            let is_slashed_reporter = report_result.is_slashed_reporter(&applicant);
            let is_slashed_stash = report_result.is_slashed_stash(&applicant);

            // 退还申述时的质押
            if is_slashed_reporter {
                Self::change_reporter_stake(applicant, staked, false)?;
            } else if is_slashed_stash {
                T::MTOps::mt_change_staked_balance(applicant, staked, false)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;
            } else {
                Self::change_committee_stake_on_report_close(vec![applicant], staked, false)
                    .map_err(|_| Error::<T>::ReduceUsedStakeFailed)?;
            }

            // 之前的结果中，报告人是否被惩罚
            let is_reporter_slashed = matches!(
                report_result.report_result,
                ReportResultType::ReportRefused | ReportResultType::ReporterNotSubmitEncryptedInfo
            );

            // 重新获得应该惩罚/奖励的委员会
            let mut should_slash = report_result.reward_committee.clone();
            for a_committee in report_result.unruly_committee.clone() {
                ItemList::add_item(&mut should_slash, a_committee)
            }
            let mut should_reward = report_result.inconsistent_committee.clone();

            // 执行与之前是否惩罚相反的质押操作
            let _ = Self::change_reporter_stake_on_report_close(
                &report_result.reporter,
                report_result.reporter_stake,
                !is_reporter_slashed,
            );

            if is_reporter_slashed {
                ItemList::add_item(&mut should_reward, report_result.reporter.clone());
            } else {
                // slash reporter
                let _ = <T as pallet::Config>::SlashAndReward::slash_and_reward(
                    vec![report_result.reporter.clone()],
                    report_result.reporter_stake,
                    should_reward.clone(),
                );
            }

            let _ = <T as pallet::Config>::SlashAndReward::slash_and_reward(
                should_slash,
                report_result.committee_stake,
                should_reward.clone(),
            );

            // remove from unhandled report result
            report_result.slash_result = MCSlashResult::Canceled;

            Self::update_unhandled_report(slashed_report_id, false, report_result.slash_exec_time);
            ReportResult::<T>::insert(slashed_report_id, report_result);
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
        CommitteeBookReport(T::AccountId, ReportId),
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
        DuplicateHash,
        NotMachineRenter,
        ReduceUsedStakeFailed,
    }
}

impl<T: Config> Pallet<T> {
    // is_add: ReporterStake改变，并reserve 一定金额
    // !is_add: ReporterStake改变，并unreserve一定金额
    fn change_reporter_stake(reporter: T::AccountId, amount: BalanceOf<T>, is_add: bool) -> DispatchResultWithPostInfo {
        let stake_params = Self::reporter_stake_params().ok_or(Error::<T>::GetStakeAmountFailed)?;
        let mut reporter_stake = Self::reporter_stake(&reporter);

        if is_add {
            ensure!(<T as Config>::Currency::can_reserve(&reporter, amount), Error::<T>::BalanceNotEnough);
            reporter_stake.staked_amount += amount;
        } else {
            ensure!(reporter_stake.staked_amount >= amount, Error::<T>::BalanceNotEnough);
            reporter_stake.staked_amount = reporter_stake.staked_amount.saturating_sub(amount);
            ensure!(reporter_stake.staked_amount >= reporter_stake.used_stake, Error::<T>::StakeNotEnough);
        }

        if reporter_stake.used_stake > Zero::zero() || is_add {
            ensure!(
                reporter_stake.staked_amount.saturating_sub(reporter_stake.used_stake)
                    >= stake_params.min_free_stake_percent * reporter_stake.staked_amount,
                Error::<T>::StakeNotEnough
            );
        }

        if is_add {
            <T as pallet::Config>::Currency::reserve(&reporter, amount).map_err(|_| Error::<T>::BalanceNotEnough)?;
            ReporterStake::<T>::insert(&reporter, reporter_stake);
            Self::deposit_event(Event::ReporterAddStake(reporter, amount));
        } else {
            <T as pallet::Config>::Currency::unreserve(&reporter, amount);
            ReporterStake::<T>::insert(&reporter, reporter_stake);
            Self::deposit_event(Event::ReporterReduceStake(reporter, amount));
        }

        Ok(().into())
    }

    // 处理用户报告逻辑
    // 记录：ReportInfo, LiveReport, ReporterReport 并支付处理所需的金额
    fn do_report_machine_fault(
        reporter: T::AccountId,
        machine_fault_type: MachineFaultType,
        report_time: Option<T::BlockNumber>,
        live_report: &mut MTLiveReportList,
        reporter_report: &mut ReporterReportList,
    ) -> DispatchResultWithPostInfo {
        // 获取处理报告需要的信息
        let stake_params = Self::reporter_stake_params().ok_or(Error::<T>::GetStakeAmountFailed)?;
        let report_id = Self::get_new_report_id();

        let report_time = report_time.unwrap_or_else(|| <frame_system::Module<T>>::block_number());

        let mut report_info = MTReportInfoDetail::new(
            reporter.clone(),
            report_time,
            machine_fault_type.clone(),
            stake_params.stake_per_report,
        );

        // 该类型错误可以由程序快速完成检测，因此可以提交并需记录machine_id
        if let MachineFaultType::RentedInaccessible(machine_id, rent_order_id) = machine_fault_type.clone() {
            report_info.machine_id = machine_id;
            report_info.rent_order_id = rent_order_id;
        }

        // 记录到 live_report & reporter_report
        ItemList::add_item(&mut live_report.bookable_report, report_id);
        ItemList::add_item(&mut reporter_report.processing_report, report_id);

        ReportInfo::<T>::insert(&report_id, report_info);

        Self::deposit_event(Event::ReportMachineFault(reporter, machine_fault_type));
        Ok(().into())
    }

    fn do_reporter_cancel_report(reporter: &T::AccountId, report_id: ReportId) {
        let mut live_report = Self::live_report();
        ItemList::rm_item(&mut live_report.bookable_report, &report_id);

        let mut reporter_report = Self::reporter_report(reporter);
        ItemList::rm_item(&mut reporter_report.processing_report, &report_id);
        ItemList::add_item(&mut reporter_report.canceled_report, report_id);

        ReporterReport::<T>::insert(reporter, reporter_report);
        LiveReport::<T>::put(live_report);
        ReportInfo::<T>::remove(&report_id);
    }

    fn do_book_reports(
        committee: T::AccountId,
        report_id: ReportId,
        report_info: &mut MTReportInfoDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        order_stake: BalanceOf<T>,
    ) {
        let now = <frame_system::Module<T>>::block_number();

        let mut live_report = Self::live_report();
        let mut committee_ops = Self::committee_ops(&committee, &report_id);
        let mut is_live_report_changed = false;
        let is_first_book = report_info.report_status == ReportStatus::Reported;
        let mft = report_info.machine_fault_type.clone();

        // 更改report_info
        ItemList::add_item(&mut report_info.booked_committee, committee.clone());
        // 记录第一个预订订单的时间, 3个小时(360个块)之后开始提交原始值
        if is_first_book {
            report_info.first_book_time = now;
            report_info.confirm_start = match report_info.machine_fault_type {
                // 如果是第一个抢单，如果是rentedInaccessible，将在5分钟后开始提交委员会的验证结果
                MachineFaultType::RentedInaccessible(..) => now + 10u32.into(),
                // 将在三个小时之后开始提交委员会的验证结果
                _ => now + THREE_HOUR.into(),
            };
        }
        report_info.report_status = match mft {
            MachineFaultType::RentedInaccessible(..) => {
                if report_info.booked_committee.len() == 3 {
                    ReportStatus::Verifying
                } else {
                    ReportStatus::WaitingBook
                }
            },
            _ => {
                // 仅在不是RentedInaccessible时进行记录，因为这些情况只能一次有一个验证委员会
                report_info.verifying_committee = Some(committee.clone());
                // 改变report状态为正在验证中，此时禁止其他委员会预订
                ReportStatus::Verifying
            },
        };

        // 更改committee_ps
        committee_ops.booked_time = now;
        committee_ops.order_status = match mft {
            MachineFaultType::RentedInaccessible(..) => MTOrderStatus::Verifying,
            _ => {
                committee_ops.staked_balance = order_stake;
                MTOrderStatus::WaitingEncrypt
            },
        };

        // 更改live_report
        if let MachineFaultType::RentedInaccessible(..) = mft {
            if report_info.booked_committee.len() == 3 {
                ItemList::rm_item(&mut live_report.bookable_report, &report_id);
                ItemList::add_item(&mut live_report.verifying_report, report_id);
                is_live_report_changed = true;
            }
        } else {
            // 从bookable_report移动到verifying_report
            ItemList::rm_item(&mut live_report.bookable_report, &report_id);
            ItemList::add_item(&mut live_report.verifying_report, report_id);
            is_live_report_changed = true;
        }

        // 更改committee_order
        let mut committee_order = Self::committee_order(&committee);
        ItemList::add_item(&mut committee_order.booked_report, report_id);

        ReportInfo::<T>::insert(&report_id, report_info);
        CommitteeOps::<T>::insert(&committee, &report_id, committee_ops);
        if is_live_report_changed {
            LiveReport::<T>::put(live_report);
        }
        CommitteeOrder::<T>::insert(&committee, committee_order);
    }

    // Hook: Summary inaccessible report
    fn summary_inaccessible_hook() -> Result<(), ()> {
        let mut live_report = Self::live_report();
        let mut verifying_report = live_report.verifying_report.clone();
        verifying_report.extend(live_report.bookable_report.clone());
        verifying_report.extend(live_report.waiting_raw_report.clone());

        for report_id in verifying_report {
            let _ = Self::summary_a_inaccessible(report_id, &mut live_report);
        }

        LiveReport::<T>::put(live_report);
        Ok(())
    }

    // - Writes:
    // ReportInfo, ReportResult, CommitteeOrder, CommitteeOps
    // LiveReport, UnhandledReportResult, ReporterReport,
    fn summary_a_inaccessible(report_id: ReportId, live_report: &mut MTLiveReportList) -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let mut report_info = Self::report_info(&report_id);

        if report_info.first_book_time == Zero::zero() {
            return Ok(());
        }

        // 仅处理Inaccessible的情况
        match report_info.machine_fault_type {
            MachineFaultType::RentedInaccessible(..) => {},
            _ => return Ok(()),
        }

        // 根据状态筛选出需要执行summary的报告
        match report_info.report_status {
            ReportStatus::Reported | ReportStatus::CommitteeConfirmed => return Ok(()),
            ReportStatus::WaitingBook | ReportStatus::Verifying => {
                // 当大于等于5分钟或者hashed的委员会已经达到3人，则更改报告状态，允许提交原始值
                if now - report_info.first_book_time >= FIVE_MINUTE.into() || report_info.hashed_committee.len() == 3 {
                    ItemList::rm_item(&mut live_report.bookable_report, &report_id); // 小于3人时处于bookable
                    ItemList::rm_item(&mut live_report.verifying_report, &report_id); // 等于3人时处于verifying
                    ItemList::add_item(&mut live_report.waiting_raw_report, report_id);

                    report_info.report_status = ReportStatus::SubmittingRaw;
                    ReportInfo::<T>::insert(report_id, report_info);
                }
                return Ok(());
            },
            ReportStatus::SubmittingRaw => {
                // 不到10分钟，且没全部提交确认，允许继续提交
                if now - report_info.first_book_time < TEN_MINUTE.into()
                    && report_info.confirmed_committee.len() < report_info.hashed_committee.len()
                {
                    return Ok(());
                }
            },
        }

        // 初始化报告结果
        let machine_info = <online_profile::Module<T>>::machines_info(&report_info.machine_id);
        let mut report_result = MTReportResultInfo {
            report_id,
            reporter: report_info.reporter.clone(),
            slash_time: now,
            slash_exec_time: now + TWO_DAY.into(),
            slash_result: MCSlashResult::Pending,
            machine_stash: machine_info.machine_stash,
            machine_id: report_info.machine_id.clone(),
            // report_result: ReportResultType::待定
            // committee_stake: 0,
            reporter_stake: report_info.reporter_stake,
            ..Default::default()
        };

        ItemList::rm_item(&mut live_report.waiting_raw_report, &report_id);

        // 修改报告人的报告记录
        let mut reporter_report = Self::reporter_report(&report_info.reporter);
        ItemList::rm_item(&mut reporter_report.processing_report, &report_id);

        // 委员会成功完成该订单，则记录；否则从记录中删除，并添加惩罚
        for a_committee in report_info.booked_committee.clone() {
            let mut committee_order = Self::committee_order(&a_committee);

            if report_info.confirmed_committee.binary_search(&a_committee).is_ok() {
                ItemList::rm_item(&mut committee_order.hashed_report, &report_id);
                ItemList::rm_item(&mut committee_order.confirmed_report, &report_id);
                ItemList::add_item(&mut committee_order.finished_report, report_id);
            } else {
                ItemList::rm_item(&mut committee_order.booked_report, &report_id);
                ItemList::rm_item(&mut committee_order.hashed_report, &report_id);

                // 添加未完成的委员会的记录，用于惩罚
                ItemList::add_item(&mut report_result.unruly_committee, a_committee.clone());
                CommitteeOps::<T>::remove(&a_committee, report_id);
            }

            CommitteeOrder::<T>::insert(&a_committee, committee_order);
        }

        // 没有委员会进行举报时，添加惩罚，重置报告状态以允许重新抢单
        // 重置report_id，因为原来的report_id已经产生了惩罚记录
        if report_info.confirmed_committee.is_empty() {
            // 调用举报函数来实现重新举报
            Self::do_report_machine_fault(
                report_info.reporter.clone(),
                report_info.machine_fault_type,
                Some(report_info.report_time),
                live_report,
                &mut reporter_report,
            )
            .map_err(|_| ())?;

            // 记录下report_result
            report_result.report_result = ReportResultType::NoConsensus;
            report_result.reporter_stake = Zero::zero();
            // Should do slash at once
            if !report_result.unruly_committee.is_empty() {
                Self::update_unhandled_report(report_id, true, report_result.slash_exec_time);
                ReportResult::<T>::insert(report_id, report_result);
            }

            ItemList::add_item(&mut reporter_report.failed_report, report_id);
            ReporterReport::<T>::insert(&report_info.reporter, reporter_report);
            return Ok(());
        }

        // 处理支持报告人的情况
        if report_info.support_committee.len() >= report_info.against_committee.len() {
            // 此时，应该支持报告人，惩罚反对的委员会

            Self::offline_machine(
                report_info.reporter.clone(),
                report_info.support_committee.clone(),
                report_info.machine_id.clone(),
                report_info.machine_fault_type.clone(),
                report_info.report_time,
            );

            ItemList::expand_to_order(&mut report_result.inconsistent_committee, report_info.against_committee.clone());
            ItemList::expand_to_order(&mut report_result.reward_committee, report_info.support_committee.clone());
            ItemList::add_item(&mut reporter_report.succeed_report, report_id);
            report_result.report_result = ReportResultType::ReportSucceed;
        } else {
            // 处理拒绝报告人的情况
            ItemList::expand_to_order(&mut report_result.inconsistent_committee, report_info.support_committee.clone());
            ItemList::expand_to_order(&mut report_result.reward_committee, report_info.against_committee.clone());
            ItemList::add_item(&mut reporter_report.failed_report, report_id);

            report_result.report_result = ReportResultType::ReportRefused;
        }

        ReporterReport::<T>::insert(&report_info.reporter, reporter_report);

        report_info.report_status = ReportStatus::CommitteeConfirmed;
        ReportInfo::<T>::insert(report_id, report_info);

        // 支持或反对，该报告都变为完成状态
        live_report.clean_unfinished_report(&report_id);
        ItemList::add_item(&mut live_report.finished_report, report_id);

        Self::update_unhandled_report(report_id, true, report_result.slash_exec_time);
        ReportResult::<T>::insert(report_id, report_result);

        Ok(())
    }

    // Hook: Summary other fault report
    fn summary_fault_hook() -> Result<(), ()> {
        let mut live_report = Self::live_report();

        // 需要检查的report可能是正在被委员会验证/仍然可以预订的状态
        let mut verifying_report = live_report.verifying_report.clone();
        verifying_report.extend(live_report.bookable_report.clone());
        let submitting_raw_report = live_report.waiting_raw_report.clone();

        for report_id in verifying_report {
            let _ = Self::summary_a_fault(report_id, &mut live_report);
        }

        // 正在提交原始值的
        for report_id in submitting_raw_report {
            let _ = Self::summary_waiting_raw(report_id, &mut live_report);
        }

        LiveReport::<T>::put(live_report);
        Ok(())
    }

    fn summary_a_fault(report_id: ReportId, live_report: &mut MTLiveReportList) -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let committee_order_stake = <T as pallet::Config>::ManageCommittee::stake_per_order().unwrap_or_default();

        let report_info = Self::report_info(&report_id);

        if report_info.first_book_time == Zero::zero() {
            return Ok(());
        }

        // 忽略掉线的类型
        if let MachineFaultType::RentedInaccessible(..) = report_info.machine_fault_type {
            return Ok(());
        };

        let mut reporter_report = Self::reporter_report(&report_info.reporter);
        let mut report_result = Self::report_result(report_id);

        // 初始化report_result
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

        if now - report_info.first_book_time < THREE_HOUR.into() {
            // 处理三小时之前的问题，报告人/委员会不按时提交信息的情况
            Self::summary_before_fault_submit_raw(
                report_id,
                now,
                live_report,
                &mut reporter_report,
                &mut report_result,
            )?;
        } else {
            // 处理超过3小时，仍然处于验证中|等待预订情况
            Self::summary_after_fault_submit_raw(report_id, now, live_report)?;
        }

        Ok(())
    }

    // 在第一个预订后，3个小时前进行检查
    fn summary_before_fault_submit_raw(
        report_id: ReportId,
        now: T::BlockNumber,

        live_report: &mut MTLiveReportList,
        reporter_report: &mut ReporterReportList,
        report_result: &mut MTReportResultInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
    ) -> Result<(), ()> {
        let mut report_info = Self::report_info(&report_id);

        match report_info.report_status {
            ReportStatus::Verifying => {},
            // Reported, WaitingBook, CommitteeConfirmed, SubmittingRaw
            _ => return Ok(()),
        }

        let verifying_committee = report_info.verifying_committee.ok_or(())?;
        let committee_ops = Self::committee_ops(&verifying_committee, &report_id);

        // 报告人没有在规定时间内提交给加密信息，则惩罚报告人到国库，不进行奖励
        if now - committee_ops.booked_time >= HALF_HOUR.into() && committee_ops.encrypted_err_info.is_none() {
            ItemList::rm_item(&mut reporter_report.processing_report, &report_id);
            ItemList::add_item(&mut reporter_report.failed_report, report_id);
            ReporterReport::<T>::insert(&report_info.reporter, reporter_report);

            // 清理存储: CommitteeOps, LiveReport, CommitteeOrder, ReporterRecord
            for a_committee in &report_info.booked_committee {
                let committee_ops = Self::committee_ops(a_committee, &report_id);
                let _ = <T as pallet::Config>::ManageCommittee::change_used_stake(
                    a_committee.clone(),
                    committee_ops.staked_balance,
                    false,
                );
                CommitteeOps::<T>::remove(a_committee, report_id);

                let mut committee_order = Self::committee_order(a_committee);
                committee_order.clean_unfinished_order(&report_id);
                CommitteeOrder::<T>::insert(a_committee, committee_order);
            }

            ItemList::rm_item(&mut live_report.verifying_report, &report_id);
            report_result.report_result = ReportResultType::ReporterNotSubmitEncryptedInfo;
            Self::update_unhandled_report(report_id, true, report_result.slash_exec_time);
            ReportResult::<T>::insert(report_id, report_result);

            return Ok(());
        }

        // 委员会没有提交Hash，删除该委员会，并惩罚
        if now - committee_ops.booked_time >= ONE_HOUR.into() {
            // 更改report_info
            report_info.verifying_committee = None;
            // 删除，以允许其他委员会进行抢单
            ItemList::rm_item(&mut report_info.booked_committee, &verifying_committee);
            ItemList::rm_item(&mut report_info.get_encrypted_info_committee, &verifying_committee);

            // 如果此时booked_committee.len() == 0；返回到最初始的状态，并允许取消报告
            if report_info.booked_committee.is_empty() {
                report_info.first_book_time = Zero::zero();
                report_info.confirm_start = Zero::zero();
                report_info.report_status = ReportStatus::Reported;
            } else {
                report_info.report_status = ReportStatus::WaitingBook
            };

            ItemList::rm_item(&mut live_report.verifying_report, &report_id);
            ItemList::add_item(&mut live_report.bookable_report, report_id);

            let mut committee_order = Self::committee_order(&verifying_committee);
            ItemList::rm_item(&mut committee_order.booked_report, &report_id);

            CommitteeOrder::<T>::insert(&verifying_committee, committee_order);
            ReportInfo::<T>::insert(report_id, report_info.clone());
            CommitteeOps::<T>::remove(&verifying_committee, &report_id);

            // NOTE: should not insert directly when summary result, but should alert exist data
            ItemList::add_item(&mut report_result.unruly_committee, verifying_committee.clone());
            Self::update_unhandled_report(report_id, true, report_result.slash_exec_time);
            ReportResult::<T>::insert(report_id, report_result);
        }
        Ok(())
    }

    fn summary_after_fault_submit_raw(
        report_id: ReportId,
        now: T::BlockNumber,
        live_report: &mut MTLiveReportList,
    ) -> Result<(), ()> {
        live_report.clean_unfinished_report(&report_id);
        ItemList::add_item(&mut live_report.waiting_raw_report, report_id);

        let mut report_info = Self::report_info(&report_id);

        if let ReportStatus::WaitingBook = report_info.report_status {
            report_info.report_status = ReportStatus::SubmittingRaw;
            ReportInfo::<T>::insert(report_id, report_info);
            return Ok(());
        }

        // 但是最后一个委员会订阅时间小于1个小时
        let verifying_committee = report_info.verifying_committee.ok_or(())?;
        let committee_ops = Self::committee_ops(&verifying_committee, &report_id);

        if now - committee_ops.booked_time < ONE_HOUR.into() {
            // 将最后一个委员会移除，不惩罚
            report_info.verifying_committee = None;
            ItemList::rm_item(&mut report_info.booked_committee, &verifying_committee);
            ItemList::rm_item(&mut report_info.get_encrypted_info_committee, &verifying_committee);

            // 从最后一个委员会的存储中删除,并退还质押
            let mut committee_order = Self::committee_order(&verifying_committee);
            committee_order.clean_unfinished_order(&report_id);
            CommitteeOrder::<T>::insert(&verifying_committee, committee_order);

            let _ = <T as pallet::Config>::ManageCommittee::change_used_stake(
                verifying_committee.clone(),
                committee_ops.staked_balance,
                false,
            );

            CommitteeOps::<T>::remove(&verifying_committee, report_id);
            ReportInfo::<T>::insert(report_id, report_info);
        }
        Ok(())
    }

    // 统计正在waiting_raw的机器
    fn summary_waiting_raw(report_id: ReportId, live_report: &mut MTLiveReportList) {
        let now = <frame_system::Module<T>>::block_number();
        let committee_order_stake = <T as pallet::Config>::ManageCommittee::stake_per_order().unwrap_or_default();

        let mut report_info = Self::report_info(&report_id);
        let mut report_result = Self::report_result(report_id);

        if report_info.first_book_time == Zero::zero() {
            return;
        }

        // 禁止对快速报告进行检查，快速报告会处理这种情况
        if let MachineFaultType::RentedInaccessible(..) = report_info.machine_fault_type {
            return;
        }

        // 未全部提交了原始信息且未达到了四个小时，需要继续等待
        if now - report_info.first_book_time < FOUR_HOUR.into()
            && report_info.hashed_committee.len() != report_info.confirmed_committee.len()
        {
            return;
        }

        let fault_report_result = Self::summary_fault_report(report_id);

        match fault_report_result.clone() {
            ReportConfirmStatus::Confirmed(support_committees, against_committee, _) => {
                // Slash against_committee and release support committee stake
                for a_committee in against_committee {
                    ItemList::add_item(&mut report_result.inconsistent_committee, a_committee.clone());

                    // 改变committee_order
                    let mut committee_order = Self::committee_order(&a_committee);
                    committee_order.clean_unfinished_order(&report_id);
                    CommitteeOrder::<T>::insert(&a_committee, committee_order);
                }
                for a_committee in support_committees.clone() {
                    ItemList::add_item(&mut report_result.reward_committee, a_committee.clone());

                    // 改变committee_order
                    let mut committee_order = Self::committee_order(&a_committee);
                    committee_order.clean_unfinished_order(&report_id);
                    ItemList::add_item(&mut committee_order.finished_report, report_id);
                    CommitteeOrder::<T>::insert(&a_committee, committee_order);
                }

                ItemList::rm_item(&mut live_report.waiting_raw_report, &report_id);
                ItemList::add_item(&mut live_report.finished_report, report_id);

                // 根据错误类型，下线机器并记录
                Self::offline_machine(
                    report_info.reporter.clone(),
                    support_committees,
                    report_info.machine_id.clone(),
                    report_info.machine_fault_type.clone(),
                    report_info.report_time,
                );

                report_result.report_result = ReportResultType::ReportSucceed;
            },
            ReportConfirmStatus::Refuse(support_committee, against_committee) => {
                // Slash support committee and release against committee stake
                report_result.i_exten_sorted(support_committee);
                report_result.r_exten_sorted(against_committee);
                report_result.report_result = ReportResultType::ReportRefused;
            },
            // 如果没有人提交，会出现NoConsensus的情况，并重新派单
            ReportConfirmStatus::NoConsensus => {
                // 所有booked_committee都应该被惩罚
                for a_committee in report_info.booked_committee.clone() {
                    CommitteeOps::<T>::remove(&a_committee, report_id);
                    let mut committee_order = Self::committee_order(&a_committee);
                    ItemList::rm_item(&mut committee_order.booked_report, &report_id);
                    ItemList::rm_item(&mut committee_order.hashed_report, &report_id);
                    CommitteeOrder::<T>::insert(&a_committee, committee_order);
                }

                // 记录unruly的委员会，两天后进行惩罚
                ItemList::expand_to_order(&mut report_result.unruly_committee, report_info.booked_committee.clone());

                let mut reporter_report = Self::reporter_report(&report_info.reporter);
                // 重新举报
                let _ = Self::do_report_machine_fault(
                    report_info.reporter.clone(),
                    report_info.machine_fault_type.clone(),
                    Some(report_info.report_time),
                    live_report,
                    &mut reporter_report,
                );
                // 重新举报时，记录报告人的质押将被重新使用，因此不再退还。
                report_result.reporter_stake = Zero::zero();
                ReporterReport::<T>::insert(&report_info.reporter, reporter_report);

                ItemList::rm_item(&mut live_report.waiting_raw_report, &report_id);
                report_result.report_result = ReportResultType::NoConsensus;
            },
        }

        // 根据报告结果，更改live_report的结果
        match fault_report_result {
            ReportConfirmStatus::Confirmed(mut sp_committees, ag_committee, ..)
            | ReportConfirmStatus::Refuse(mut sp_committees, ag_committee, ..) => {
                ItemList::rm_item(&mut live_report.waiting_raw_report, &report_id);
                ItemList::add_item(&mut live_report.finished_report, report_id);

                sp_committees.extend(ag_committee);
                for a_committee in sp_committees {
                    let mut committee_order = Self::committee_order(&a_committee);
                    ItemList::rm_item(&mut committee_order.confirmed_report, &report_id);
                    ItemList::add_item(&mut committee_order.finished_report, report_id);
                    CommitteeOrder::<T>::insert(&a_committee, committee_order);
                }

                report_result.reporter_stake = report_info.reporter_stake;
            },
            ReportConfirmStatus::NoConsensus => {
                report_result.reporter_stake = Zero::zero();
            },
        }

        report_result = MTReportResultInfo {
            report_id,
            reporter: report_info.reporter.clone(),
            committee_stake: committee_order_stake,
            slash_time: now,
            slash_exec_time: now + TWO_DAY.into(),

            slash_result: MCSlashResult::Pending,

            ..report_result
        };

        Self::update_unhandled_report(report_id, true, now + TWO_DAY.into());

        if report_info.report_status != ReportStatus::Reported {
            report_info.report_status = ReportStatus::CommitteeConfirmed;
        }
        ReportResult::<T>::insert(report_id, report_result);
        ReportInfo::<T>::insert(report_id, report_info);
    }

    // Summary committee's handle result depend on support & against votes
    fn summary_fault_report(report_id: ReportId) -> ReportConfirmStatus<T::AccountId> {
        let report_info = Self::report_info(&report_id);

        if report_info.confirmed_committee.is_empty() {
            return ReportConfirmStatus::NoConsensus;
        }

        if report_info.support_committee.len() >= report_info.against_committee.len() {
            return ReportConfirmStatus::Confirmed(
                report_info.support_committee,
                report_info.against_committee,
                report_info.err_info,
            );
        }
        ReportConfirmStatus::Refuse(report_info.support_committee, report_info.against_committee)
    }

    // 根据错误类型，匹配到新的错误，并进行下线机器的操作
    fn offline_machine(
        reporter: T::AccountId,
        support_committee: Vec<T::AccountId>,
        machine_id: MachineId,
        raw_fault_type: MachineFaultType,
        report_time: T::BlockNumber,
    ) {
        let fault_type = match raw_fault_type {
            MachineFaultType::RentedInaccessible(..) => online_profile::OPSlashReason::RentedInaccessible(report_time),
            MachineFaultType::RentedHardwareMalfunction(..) => {
                online_profile::OPSlashReason::RentedHardwareMalfunction(report_time)
            },
            MachineFaultType::RentedHardwareCounterfeit(..) => {
                online_profile::OPSlashReason::RentedHardwareCounterfeit(report_time)
            },
            MachineFaultType::OnlineRentFailed(..) => online_profile::OPSlashReason::OnlineRentFailed(report_time),
        };

        T::MTOps::mt_machine_offline(reporter, support_committee, machine_id, fault_type);
    }
}
