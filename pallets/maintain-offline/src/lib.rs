// 报告机器离线
// 报告人发报告内容的Hash到链上，等待委员会抢单
// 委员会一旦抢单，报告人将信息发给委员会，委员会需要在5分钟内验证完毕。
// 任何人都可以进行抢单，抢单后不提交结果，则作废
// 验证确认问题之后，机器从被举报时开始计算离线时间

#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use codec::EncodeLike;
use frame_support::{
    pallet_prelude::*,
    traits::{Currency, LockableCurrency},
};
use frame_system::pallet_prelude::*;
use maintain_committee::{
    MTLiveReportList, MTReportInfoDetail, ReportId, ReportStatus, ReportType, ReporterRecord,
};
use online_profile_machine::{DbcPrice, ManageCommittee};
use sp_std::vec::Vec;

pub use pallet::*;

type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub type MachineId = Vec<u8>;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + generic_func::Config {
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type DbcPrice: DbcPrice<BalanceOf = BalanceOf<Self>>;
        type ManageCommittee: ManageCommittee<
            AccountId = Self::AccountId,
            BalanceOf = BalanceOf<Self>,
        >;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    #[pallet::storage]
    #[pallet::getter(fn live_report)]
    pub(super) type LiveReport<T: Config> = StorageValue<_, MTLiveReportList, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_report_id)]
    pub(super) type NextReportId<T: Config> = StorageValue<_, ReportId, ValueQuery>;

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

    // 报告人最小质押，默认100RMB等值DBC
    #[pallet::storage]
    #[pallet::getter(fn reporter_report_stake)]
    pub(super) type ReporterReportStake<T: Config> = StorageValue<_, u64, ValueQuery>;

    // 查询报告人报告的机器
    #[pallet::storage]
    #[pallet::getter(fn reporter_report)]
    pub(super) type ReporterReport<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, ReporterRecord, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 报告机器离线
        #[pallet::weight(10000)]
        pub fn report_machine_offline(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            let report_time = <frame_system::Module<T>>::block_number();
            let report_id = Self::get_new_report_id();

            let reporter_report_stake = Self::reporter_report_stake();
            let reporter_stake_need = T::DbcPrice::get_dbc_amount_by_value(reporter_report_stake)
                .ok_or(Error::<T>::GetStakeAmountFailed)?;

            <T as pallet::Config>::ManageCommittee::change_stake(
                &reporter,
                reporter_stake_need,
                true,
            )
            .map_err(|_| Error::<T>::StakeFailed)?;

            // 支付10个DBC
            <generic_func::Module<T>>::pay_fixed_tx_fee(reporter.clone())
                .map_err(|_| Error::<T>::PayTxFeeFailed)?;

            let mut live_report = Self::live_report();
            if let Err(index) = live_report.bookable_report.binary_search(&report_id) {
                live_report.bookable_report.insert(index, report_id);
            }
            LiveReport::<T>::put(live_report);

            ReportInfo::<T>::insert(
                &report_id,
                MTReportInfoDetail {
                    reporter: reporter.clone(),
                    report_time,
                    machine_id,
                    reporter_stake: reporter_stake_need,
                    report_status: ReportStatus::Reported,
                    report_type: ReportType::MachineOffline,
                    ..Default::default()
                },
            );

            // 记录到报告人的存储中
            let mut reporter_report = Self::reporter_report(&reporter);
            if let Err(index) = reporter_report.reported_id.binary_search(&report_id) {
                reporter_report.reported_id.insert(index, report_id);
            }
            ReporterReport::<T>::insert(&reporter, reporter_report);

            Ok(().into())
        }

        // 预订离线的机器订单
        #[pallet::weight(10000)]
        pub fn book_offline_order(
            origin: OriginFor<T>,
            _report_id: ReportId,
        ) -> DispatchResultWithPostInfo {
            let _committee = ensure_signed(origin)?;
            let _now = <frame_system::Module<T>>::block_number();
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        BondMachine(T::AccountId, MachineId, BalanceOf<T>),
        AddBonded(T::AccountId, MachineId, BalanceOf<T>),
        RemoveBonded(T::AccountId, MachineId, BalanceOf<T>),
        DonationReceived(T::AccountId, BalanceOf<T>, BalanceOf<T>),
        FundsAllocated(T::AccountId, BalanceOf<T>, BalanceOf<T>),
        Withdrawn(T::AccountId, MachineId, BalanceOf<T>),
        ClaimRewards(T::AccountId, MachineId, BalanceOf<T>),
        ReporterStake(T::AccountId, MachineId, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        GetStakeAmountFailed,
        PayTxFeeFailed,
        StakeFailed,
    }
}

impl<T: Config> Pallet<T> {
    fn get_new_report_id() -> ReportId {
        let report_id = Self::next_report_id();
        NextReportId::<T>::put(report_id + 1);
        return report_id;
    }
}
