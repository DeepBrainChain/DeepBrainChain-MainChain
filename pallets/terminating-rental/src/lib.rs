#![cfg_attr(not(feature = "std"), no_std)]

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

mod online_verify_slash;
mod report_machine_fault;
mod rpc;
pub mod rpc_types;
mod types;

use frame_support::{
    dispatch::{DispatchResult, DispatchResultWithPostInfo},
    pallet_prelude::*,
    traits::{Currency, ExistenceRequirement::KeepAlive, OnUnbalanced, ReservableCurrency},
};
use sp_runtime::{
    traits::{CheckedAdd, CheckedMul, CheckedSub, SaturatedConversion, Saturating, Zero},
    Perbill,
};
use sp_std::{prelude::*, str, vec::Vec};

use dbc_support::{
    machine_type::{CommitteeUploadInfo, MachineStatus, StakerCustomizeInfo},
    rental_type::{MachineGPUOrder, RentOrderDetail, RentStatus},
    traits::{DbcPrice, GNOps, ManageCommittee},
    EraIndex, MachineId, RentOrderId, SlashId, TWO_DAY,
};
use generic_func::ItemList;

/// 36 hours divide into 9 intervals for verification
pub const DISTRIBUTION: u32 = 9;
/// After order distribution 36 hours, allow committee submit raw info
pub const SUBMIT_HASH_END: u32 = 4320;
/// After order distribution 36 hours, allow committee submit raw info
pub const SUBMIT_RAW_START: u32 = 4320;
/// Summary committee's opinion after 48 hours
pub const SUBMIT_RAW_END: u32 = 5760;
/// 等待30个块(15min)，用户确认是否租用成功
pub const WAITING_CONFIRMING_DELAY: u32 = 30;
// /// 1天按照2880个块
// pub const BLOCK_PER_DAY: u32 = 2880;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

pub use pallet::*;
pub use types::*;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    use crate::NegativeImbalanceOf;
    use frame_system::pallet_prelude::*;
    use sp_core::H256;

    #[pallet::config]
    pub trait Config: frame_system::Config + generic_func::Config + committee::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
        type ManageCommittee: ManageCommittee<
            AccountId = Self::AccountId,
            Balance = BalanceOf<Self>,
        >;
        type DbcPrice: DbcPrice<Balance = BalanceOf<Self>>;
        type SlashAndReward: GNOps<AccountId = Self::AccountId, Balance = BalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    // 上卡验证前，需要质押保证金
    #[pallet::storage]
    #[pallet::getter(fn online_deposit)]
    pub(super) type OnlineDeposit<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn stash_controller)]
    pub(super) type StashController<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    #[pallet::storage]
    #[pallet::getter(fn controller_stash)]
    pub(super) type ControllerStash<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    /// Server rooms in stash account
    #[pallet::storage]
    #[pallet::getter(fn stash_server_rooms)]
    pub(super) type StashServerRooms<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Vec<H256>, ValueQuery>;

    /// Statistics of stash account
    #[pallet::storage]
    #[pallet::getter(fn stash_machines)]
    pub(super) type StashMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, IRStashMachine<BalanceOf<T>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn offline_machines)]
    pub(super) type OfflineMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, T::BlockNumber, Vec<MachineId>, ValueQuery>;

    /// 资金账户的质押总计
    #[pallet::storage]
    #[pallet::getter(fn stash_stake)]
    pub(super) type StashStake<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    /// 系统中存储有数据的机器
    #[pallet::storage]
    #[pallet::getter(fn live_machines)]
    pub type LiveMachines<T: Config> = StorageValue<_, IRLiveMachine, ValueQuery>;

    /// Detail info of machines
    #[pallet::storage]
    #[pallet::getter(fn machines_info)]
    pub type MachinesInfo<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        IRMachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn stake_per_gpu)]
    pub(super) type StakePerGPU<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// 记录机器被租用的GPU个数
    #[pallet::storage]
    #[pallet::getter(fn machine_rented_gpu)]
    pub type MachineRentedGPU<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, u32, ValueQuery>;

    // 存储用户订阅的不同确认阶段的机器
    #[pallet::storage]
    #[pallet::getter(fn committee_machine)]
    pub(super) type CommitteeMachine<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, IRCommitteeMachineList, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn machine_committee)]
    pub(super) type MachineCommittee<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        IRMachineCommitteeList<T::AccountId, T::BlockNumber>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn machine_submited_hash)]
    pub(super) type MachineSubmitedHash<T> =
        StorageMap<_, Blake2_128Concat, MachineId, Vec<[u8; 16]>, ValueQuery>;

    // 验证机器上线的委员会操作
    #[pallet::storage]
    #[pallet::getter(fn committee_online_ops)]
    pub(super) type CommitteeOnlineOps<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        MachineId,
        IRCommitteeOnlineOps<T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    // 存储用户当前租用的机器列表
    #[pallet::storage]
    #[pallet::getter(fn user_rented)]
    pub(super) type UserRented<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Vec<RentOrderId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn machine_rent_order)]
    pub(super) type MachineRentOrder<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, MachineGPUOrder, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_rent_id)]
    pub(super) type NextRentId<T: Config> = StorageValue<_, RentOrderId, ValueQuery>;

    // 用户当前租用的某个机器的详情
    // 记录每个租用记录
    #[pallet::storage]
    #[pallet::getter(fn rent_order)]
    pub type RentOrder<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        RentOrderId,
        RentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    // 等待用户确认租用成功的机器
    #[pallet::storage]
    #[pallet::getter(fn pending_confirming)]
    pub type PendingConfirming<T: Config> =
        StorageMap<_, Blake2_128Concat, T::BlockNumber, Vec<RentOrderId>, ValueQuery>;

    // 记录每个区块将要结束租用的机器
    #[pallet::storage]
    #[pallet::getter(fn pending_rent_ending)]
    pub(super) type PendingRentEnding<T: Config> =
        StorageMap<_, Blake2_128Concat, T::BlockNumber, Vec<RentOrderId>, ValueQuery>;

    #[pallet::type_value]
    pub(super) fn MaximumRentalDurationDefault<T: Config>() -> EraIndex {
        60
    }

    // 最大租用/续租用时间
    #[pallet::storage]
    #[pallet::getter(fn maximum_rental_duration)]
    pub(super) type MaximumRentalDuration<T: Config> =
        StorageValue<_, EraIndex, ValueQuery, MaximumRentalDurationDefault<T>>;

    // 可打断式更新租金折扣，可设置与标准GPU机器不同的租金水平
    /// A standard example for rent fee calculation(price: USD*10^6)
    #[pallet::storage]
    #[pallet::getter(fn standard_gpu_point_price)]
    pub(super) type StandardGPUPointPrice<T: Config> =
        StorageValue<_, dbc_support::machine_type::StandardGpuPointPrice>;

    // 存储每个用户在该模块中的总质押量
    #[pallet::storage]
    #[pallet::getter(fn renter_total_stake)]
    pub(super) type RenterTotalStake<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn rented_finished)]
    pub(super) type RentedFinished<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, T::AccountId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_slash_id)]
    pub(super) type NextSlashId<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pending_online_slash)]
    pub(super) type PendingOnlineSlash<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        SlashId,
        IRPendingOnlineSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    // #[pallet::storage]
    // #[pallet::getter(fn pending_slash_review)]
    // pub(super) type PendingSlashReview<T: Config> = StorageMap<
    //     _,
    //     Blake2_128Concat,
    //     SlashId,
    //     IRPendingSlashReviewInfo<T::AccountId, BalanceOf<T>, T::BlockNumber>,
    //     ValueQuery,
    // >;

    #[pallet::storage]
    #[pallet::getter(fn unhandled_online_slash)]
    pub(super) type UnhandledOnlineSlash<T: Config> = StorageValue<_, Vec<SlashId>, ValueQuery>;

    /// 系统中还未完成的举报订单
    #[pallet::storage]
    #[pallet::getter(fn live_report)]
    pub(super) type LiveReport<T: Config> = StorageValue<_, IRLiveReportList, ValueQuery>;

    /// 系统中还未完成的订单
    // 通过报告单据ID，查询报告的机器的信息(委员会抢单信息)
    #[pallet::storage]
    #[pallet::getter(fn report_info)]
    pub(super) type ReportInfo<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        ReportId,
        IRReportInfoDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    /// Report record for reporter
    #[pallet::storage]
    #[pallet::getter(fn reporter_report)]
    pub(super) type ReporterReport<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, IRReporterReportList, ValueQuery>;

    // TODO: 增加set函数
    #[pallet::storage]
    #[pallet::getter(fn reporter_stake_params)]
    pub(super) type ReporterStakeParams<T: Config> =
        StorageValue<_, IRReporterStakeParamsInfo<BalanceOf<T>>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_report_id)]
    pub(super) type NextReportId<T: Config> = StorageValue<_, ReportId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn reporter_stake)]
    pub(super) type ReporterStake<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        IRReporterStakeInfo<BalanceOf<T>>,
        ValueQuery,
    >;

    // 委员会查询自己的抢单信息
    #[pallet::storage]
    #[pallet::getter(fn committee_report_order)]
    pub(super) type CommitteeReportOrder<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, IRCommitteeReportOrderList, ValueQuery>;

    // 存储委员会对单台机器的操作记录
    #[pallet::storage]
    #[pallet::getter(fn committee_report_ops)]
    pub(super) type CommitteeReportOps<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        ReportId,
        IRCommitteeReportOpsDetail<T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn report_result)]
    pub(super) type ReportResult<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        ReportId,
        IRReportResultInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn unhandled_report_result)]
    pub(super) type UnhandledReportResult<T: Config> =
        StorageMap<_, Blake2_128Concat, T::BlockNumber, Vec<ReportId>, ValueQuery>;

    // 机器主动下线后，记录机器下线超过最大值{5,10天}后，需要立即执行的惩罚
    #[pallet::storage]
    #[pallet::getter(fn pending_offline_slash)]
    pub(super) type PendingOfflineSlash<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::BlockNumber,
        Blake2_128Concat,
        MachineId,
        // 记录机器举报人，当前租用人
        (Option<T::AccountId>, Vec<T::AccountId>),
        ValueQuery,
    >;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_n: BlockNumberFor<T>) -> frame_support::weights::Weight {
            Self::check_and_exec_pending_slash();

            Self::summary_fault_report_hook();
            0
        }

        fn on_finalize(_block_number: T::BlockNumber) {
            Self::statistic_online_verify();
            Self::distribute_machines();

            // Self::check_machine_starting_status();
            Self::check_if_rent_finished();
            // TODO:  检查OfflineMachines是否到达了10天
            let _ = Self::check_if_offline_timeout();

            let _ = Self::exec_report_slash();
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(0)]
        // 设置每张卡质押数量
        pub fn set_stake_per_gpu(
            origin: OriginFor<T>,
            stake_per_gpu: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StakePerGPU::<T>::put(stake_per_gpu);
            Ok(().into())
        }

        // 需要质押10000DBC作为保证金，验证通过保证金解锁
        #[pallet::weight(0)]
        pub fn set_online_deposit(
            origin: OriginFor<T>,
            deposit: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            OnlineDeposit::<T>::put(deposit);
            Ok(().into())
        }

        // 设置特定GPU标准算力与对应的每天租用价格
        #[pallet::weight(0)]
        pub fn set_standard_gpu_point_price(
            origin: OriginFor<T>,
            point_price: dbc_support::machine_type::StandardGpuPointPrice,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StandardGPUPointPrice::<T>::put(point_price);
            Ok(().into())
        }

        // 资金账户设置控制账户
        #[pallet::weight(10000)]
        pub fn set_controller(
            origin: OriginFor<T>,
            controller: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let stash = ensure_signed(origin)?;

            // Don't allow multiple stash have same controller
            ensure!(
                !<ControllerStash<T>>::contains_key(&controller),
                Error::<T>::AlreadyController
            );
            ensure!(!<StashController<T>>::contains_key(&stash), Error::<T>::AlreadyController);

            StashController::<T>::insert(stash.clone(), controller.clone());
            ControllerStash::<T>::insert(controller.clone(), stash.clone());

            Self::deposit_event(Event::ControllerStashBonded(controller, stash));
            Ok(().into())
        }

        // Controller generate new server room id, record to stash account
        #[pallet::weight(10000)]
        pub fn gen_server_room(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashBond)?;
            Self::pay_fixed_tx_fee(controller.clone())?;

            StashServerRooms::<T>::mutate(stash, |server_rooms| {
                let new_server_room = <generic_func::Module<T>>::random_server_room();
                ItemList::add_item(server_rooms, new_server_room);

                Self::deposit_event(Event::ServerRoomGenerated(controller, new_server_room));
                Ok(().into())
            })
        }

        // - Writes: LiveMachine, StashMachines, MachineInfo,
        // StashStake, Balance
        #[pallet::weight(10000)]
        pub fn bond_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            msg: Vec<u8>,
            sig: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashBond)?;
            let now = <frame_system::Module<T>>::block_number();
            let online_deposit = Self::online_deposit();

            ensure!(!MachinesInfo::<T>::contains_key(&machine_id), Error::<T>::MachineIdExist);
            // 检查签名是否正确
            Self::check_bonding_msg(stash.clone(), machine_id.clone(), msg, sig)?;
            // 需要质押10000DBC作为保证金，验证通过保证金解锁
            Self::change_stash_total_stake(stash.clone(), online_deposit, true)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;

            LiveMachines::<T>::mutate(|live_machines| {
                live_machines.bond_machine(machine_id.clone())
            });
            StashMachines::<T>::mutate(&stash, |stash_machines| {
                stash_machines.bond_machine(machine_id.clone())
            });
            MachinesInfo::<T>::insert(
                &machine_id,
                IRMachineInfo::bond_machine(stash, now, online_deposit),
            );

            Ok(().into())
        }

        // - Write: LiveMachine, MachinesInfo
        #[pallet::weight(10000)]
        pub fn add_machine_info(
            origin: OriginFor<T>,
            machine_id: MachineId,
            add_machine_info: StakerCustomizeInfo,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let mut machine_info = Self::machines_info(&machine_id);

            // 查询机器Id是否在该账户的控制下
            ensure!(
                Self::stash_controller(&machine_info.machine_stash) == Some(controller),
                Error::<T>::NotMachineController
            );

            // 确保机房ID存在
            let stash_server_rooms = Self::stash_server_rooms(&machine_info.machine_stash);
            ensure!(
                stash_server_rooms.binary_search(&add_machine_info.server_room).is_ok(),
                Error::<T>::ServerRoomNotFound
            );

            machine_info
                .add_machine_info(add_machine_info)
                .map_err::<Error<T>, _>(Into::into)?;

            LiveMachines::<T>::mutate(|live_machines| {
                live_machines.add_machine_info(machine_id.clone())
            });
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::MachineInfoAdded(machine_id));
            Ok(().into())
        }

        // - Writes: CommitteeMachine, CommitteeOps, MachineSubmitedHash, MachineCommittee
        #[pallet::weight(10000)]
        pub fn submit_confirm_hash(
            origin: OriginFor<T>,
            machine_id: MachineId,
            hash: [u8; 16],
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut machine_submited_hash = Self::machine_submited_hash(&machine_id);
            ensure!(machine_submited_hash.binary_search(&hash).is_err(), Error::<T>::DuplicateHash);
            ItemList::add_item(&mut machine_submited_hash, hash);

            let mut machine_committee = Self::machine_committee(&machine_id);
            machine_committee
                .submit_hash(committee.clone())
                .map_err::<Error<T>, _>(Into::into)?;

            // 更新存储
            CommitteeMachine::<T>::mutate(&committee, |committee_machine| {
                committee_machine.submit_hash(machine_id.clone())
            });
            CommitteeOnlineOps::<T>::mutate(&committee, &machine_id, |committee_ops| {
                committee_ops.submit_hash(now, hash)
            });
            MachineSubmitedHash::<T>::insert(&machine_id, machine_submited_hash);
            MachineCommittee::<T>::insert(&machine_id, machine_committee);

            Self::deposit_event(Event::AddConfirmHash(committee, hash));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn submit_confirm_raw(
            origin: OriginFor<T>,
            machine_info_detail: CommitteeUploadInfo,
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let machine_id = machine_info_detail.machine_id.clone();

            let mut machine_committee = Self::machine_committee(&machine_id);
            let mut committee_machine = Self::committee_machine(&committee);
            let mut committee_ops = Self::committee_online_ops(&committee, &machine_id);

            ensure!(
                machine_info_detail.hash() == committee_ops.confirm_hash,
                Error::<T>::InfoNotFeatHash
            );

            committee_machine
                .submit_raw(machine_id.clone())
                .map_err::<Error<T>, _>(Into::into)?;
            machine_committee
                .submit_raw(now, committee.clone())
                .map_err::<Error<T>, _>(Into::into)?;
            committee_ops.submit_raw(now, machine_info_detail);

            CommitteeMachine::<T>::insert(&committee, committee_machine);
            MachineCommittee::<T>::insert(&machine_id, machine_committee);
            CommitteeOnlineOps::<T>::insert(&committee, &machine_id, committee_ops);

            Self::deposit_event(Event::AddConfirmRaw(committee, machine_id));
            Ok(().into())
        }

        /// 用户租用机器（按分钟租用）
        #[pallet::weight(10000)]
        pub fn rent_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            rent_gpu_num: u32,
            duration: T::BlockNumber,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let machine_info = Self::machines_info(&machine_id);
            let machine_rented_gpu = Self::machine_rented_gpu(&machine_id);
            let gpu_num = machine_info.gpu_num();
            // 检查还有空闲的GPU
            ensure!(rent_gpu_num + machine_rented_gpu <= gpu_num, Error::<T>::GPUNotEnough);
            // 只允许半小时整数倍的租用
            ensure!(
                duration % 60u32.into() == Zero::zero(),
                Error::<T>::OnlyAllowIntegerMultipleOfHour
            );

            // 检查machine_id状态是否可以租用
            ensure!(machine_info.can_rent(), Error::<T>::MachineNotRentable);

            // 最大租用时间限制MaximumRentalDuration
            let duration = duration.min((Self::maximum_rental_duration() * 24 * 60).into());

            // NOTE: 用户提交订单，需要扣除10个DBC
            Self::pay_fixed_tx_fee(renter.clone())?;

            // 获得machine_price(每天的价格)
            // 根据租用GPU数量计算价格
            let machine_price =
                Self::get_machine_price(machine_info.calc_point(), rent_gpu_num, gpu_num)
                    .ok_or(Error::<T>::GetMachinePriceFailed)?;

            // 根据租用时长计算rent_fee
            let rent_fee_value = machine_price
                .checked_mul(duration.saturated_into::<u64>())
                .ok_or(Error::<T>::Overflow)?
                .checked_div(24 * 60 * 2)
                .ok_or(Error::<T>::Overflow)?;
            let rent_fee = <T as Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value)
                .ok_or(Error::<T>::Overflow)?;

            // 获取用户租用的结束时间(块高)
            let rent_end = now.checked_add(&duration).ok_or(Error::<T>::Overflow)?;

            // 质押用户的资金，并修改机器状态
            Self::change_renter_total_stake(&renter, rent_fee, true)
                .map_err(|_| Error::<T>::InsufficientValue)?;

            let rent_id = Self::get_new_rent_id();

            let mut machine_rent_order = Self::machine_rent_order(&machine_id);
            let rentable_gpu_index = machine_rent_order.gen_rentable_gpu(rent_gpu_num, gpu_num);
            ItemList::add_item(&mut machine_rent_order.rent_order, rent_id);
            MachineRentOrder::<T>::insert(&machine_id, machine_rent_order);

            RentOrder::<T>::insert(
                &rent_id,
                RentOrderDetail::new(
                    machine_id.clone(),
                    renter.clone(),
                    now,
                    rent_end,
                    rent_fee,
                    rent_gpu_num,
                    rentable_gpu_index,
                ),
            );

            // 改变online_profile状态，影响机器佣金
            Self::change_machine_status_on_rent_start(&machine_id, rent_gpu_num);

            UserRented::<T>::mutate(&renter, |user_rented| {
                ItemList::add_item(user_rented, rent_id);
            });
            PendingRentEnding::<T>::mutate(rent_end, |pending_rent_ending| {
                ItemList::add_item(pending_rent_ending, rent_id);
            });
            PendingConfirming::<T>::mutate(
                now + WAITING_CONFIRMING_DELAY.into(),
                |pending_confirming| {
                    ItemList::add_item(pending_confirming, rent_id);
                },
            );

            Self::deposit_event(Event::RentBlockNum(
                rent_id,
                renter,
                machine_id,
                rent_fee,
                duration.into(),
                gpu_num,
            ));
            Ok(().into())
        }

        /// 用户在租用15min(30个块)内确认机器租用成功
        #[pallet::weight(10000)]
        pub fn confirm_rent(
            origin: OriginFor<T>,
            rent_id: RentOrderId,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut order_info = Self::rent_order(&rent_id);
            let machine_id = order_info.machine_id.clone();
            ensure!(order_info.renter == renter, Error::<T>::NoOrderExist);
            ensure!(
                order_info.rent_status == RentStatus::WaitingVerifying,
                Error::<T>::NoOrderExist
            );

            // 不能超过15分钟
            let machine_start_duration =
                now.checked_sub(&order_info.rent_start).ok_or(Error::<T>::Overflow)?;
            ensure!(
                machine_start_duration <= WAITING_CONFIRMING_DELAY.into(),
                Error::<T>::ExpiredConfirm
            );

            let machine_info = Self::machines_info(&machine_id);
            ensure!(
                machine_info.machine_status == MachineStatus::Rented,
                Error::<T>::StatusNotAllowed
            );

            // 在stake_amount设置0前记录，用作事件
            let rent_fee = order_info.stake_amount;
            let rent_duration = order_info.rent_end - order_info.rent_start;

            order_info.confirm_rent(now);

            // 改变online_profile状态
            Self::change_machine_status_on_confirmed(&machine_id, renter.clone());

            // TODO: 当为空时，删除
            PendingConfirming::<T>::mutate(
                order_info.rent_start + WAITING_CONFIRMING_DELAY.into(),
                |pending_confirming| {
                    ItemList::rm_item(pending_confirming, &rent_id);
                },
            );
            RentOrder::<T>::insert(&rent_id, order_info);

            Self::deposit_event(Event::ConfirmReletBlockNum(
                renter,
                machine_id,
                rent_fee,
                rent_duration,
            ));
            Ok(().into())
        }

        /// 用户续租(按分钟续租)
        #[pallet::weight(10000)]
        pub fn relet_machine(
            origin: OriginFor<T>,
            rent_id: RentOrderId,
            duration: T::BlockNumber,
        ) -> DispatchResultWithPostInfo {
            let renter = ensure_signed(origin)?;
            let mut order_info = Self::rent_order(&rent_id);
            let pre_rent_end = order_info.rent_end;
            let machine_id = order_info.machine_id.clone();
            let gpu_num = order_info.gpu_num;

            ensure!(
                duration % 60u32.into() == Zero::zero(),
                Error::<T>::OnlyAllowIntegerMultipleOfHour
            );
            ensure!(order_info.renter == renter, Error::<T>::NoOrderExist);
            ensure!(order_info.rent_status == RentStatus::Renting, Error::<T>::NoOrderExist);

            let machine_info = Self::machines_info(&machine_id);
            let calc_point = machine_info.calc_point();

            // 确保租用时间不超过设定的限制，计算最多续费租用到
            let now = <frame_system::Module<T>>::block_number();
            // 最大结束块高为 今天租用开始的时间 + 60天
            // 2880 块/天 * 60 days
            let max_rent_end =
                now.checked_add(&(2880u32 * 60).into()).ok_or(Error::<T>::Overflow)?;
            let wanted_rent_end = pre_rent_end + duration;

            // 计算实际续租了多久 (块高)
            let add_duration: T::BlockNumber =
                if max_rent_end >= wanted_rent_end { duration } else { (2880u32 * 60).into() };

            if add_duration == Zero::zero() {
                return Ok(().into())
            }

            // 计算rent_fee
            let machine_price =
                Self::get_machine_price(calc_point, gpu_num, machine_info.gpu_num())
                    .ok_or(Error::<T>::GetMachinePriceFailed)?;
            let rent_fee_value = machine_price
                .checked_mul(add_duration.saturated_into::<u64>())
                .ok_or(Error::<T>::Overflow)?
                .checked_div(2880)
                .ok_or(Error::<T>::Overflow)?;
            let rent_fee = <T as Config>::DbcPrice::get_dbc_amount_by_value(rent_fee_value)
                .ok_or(Error::<T>::Overflow)?;

            // 检查用户是否有足够的资金，来租用机器
            let user_balance = <T as Config>::Currency::free_balance(&renter);
            ensure!(rent_fee < user_balance, Error::<T>::InsufficientValue);

            // 质押用户的资金，并修改机器状态
            Self::change_renter_total_stake(&renter, rent_fee, true)
                .map_err(|_| Error::<T>::InsufficientValue)?;

            // 获取用户租用的结束时间
            order_info.rent_end =
                order_info.rent_end.checked_add(&add_duration).ok_or(Error::<T>::Overflow)?;
            order_info.stake_amount =
                order_info.stake_amount.checked_add(&rent_fee).ok_or(Error::<T>::Overflow)?;

            PendingRentEnding::<T>::mutate(pre_rent_end, |pre_pending_rent_ending| {
                ItemList::rm_item(pre_pending_rent_ending, &rent_id);
            });
            PendingRentEnding::<T>::mutate(order_info.rent_end, |pending_rent_ending| {
                ItemList::add_item(pending_rent_ending, rent_id);
            });
            RentOrder::<T>::insert(&rent_id, order_info);

            Self::deposit_event(Event::ReletBlockNum(
                rent_id,
                renter,
                machine_id,
                rent_fee,
                add_duration,
                gpu_num,
            ));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn machine_offline(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let mut machine_info = Self::machines_info(&machine_id);
            ensure!(
                Self::stash_controller(&machine_info.machine_stash) == Some(controller),
                Error::<T>::NotMachineController
            );

            let now = <frame_system::Module<T>>::block_number();
            let machine_rent_order = Self::machine_rent_order(&machine_id);

            machine_info.machine_offline(now);
            for rent_id in machine_rent_order.rent_order {
                let rent_order = Self::rent_order(rent_id);

                // 根据时间(小时向下取整)计算需要的租金
                let rent_duration =
                    now.saturating_sub(rent_order.rent_start) / 120u32.into() * 120u32.into();
                let rent_fee = Perbill::from_rational_approximation(
                    rent_duration,
                    rent_order.rent_end - rent_order.rent_start,
                ) * rent_order.stake_amount;

                Self::pay_rent_fee(&rent_order, rent_fee, machine_id.clone())?;

                RentOrder::<T>::remove(rent_id);
            }
            MachineRentOrder::<T>::remove(&machine_id);

            // 记录到一个变量中，检查是否已经连续下线超过了10天
            OfflineMachines::<T>::mutate(now + 28800u32.into(), |offline_machines| {
                ItemList::add_item(offline_machines, machine_id.clone());
            });

            MachinesInfo::<T>::insert(&machine_id, machine_info);
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn machine_online(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let mut machine_info = Self::machines_info(&machine_id);
            ensure!(
                Self::stash_controller(&machine_info.machine_stash) == Some(controller),
                Error::<T>::NotMachineController
            );

            if let MachineStatus::StakerReportOffline(offline_expire_time, _) =
                machine_info.machine_status
            {
                let mut offline_machines = Self::offline_machines(offline_expire_time);
                ItemList::rm_item(&mut offline_machines, &machine_id);
                if !offline_machines.is_empty() {
                    OfflineMachines::<T>::insert(offline_expire_time, offline_machines);
                } else {
                    OfflineMachines::<T>::remove(offline_expire_time);
                }

                machine_info.machine_status = MachineStatus::Online;
                MachinesInfo::<T>::insert(machine_id, machine_info);
                Ok(().into())
            } else {
                return Err(Error::<T>::StatusNotAllowed.into())
            }
        }

        // 满1年，机器可以退出，并退还质押币
        #[pallet::weight(10000)]
        pub fn machine_exit(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let machine_info = Self::machines_info(&machine_id);
            ensure!(
                Self::stash_controller(&machine_info.machine_stash) == Some(controller),
                Error::<T>::NotMachineController
            );

            let now = <frame_system::Module<T>>::block_number();
            ensure!(
                now.saturating_sub(machine_info.online_height) >= (365 * 2880u32).into(),
                Error::<T>::TimeNotAllow
            );

            let machine_rent_order = Self::machine_rent_order(&machine_id);

            for rent_id in machine_rent_order.rent_order {
                let rent_order = Self::rent_order(rent_id);

                // 根据时间(小时向下取整)计算需要的租金
                let rent_duration =
                    now.saturating_sub(rent_order.rent_start) / 120u32.into() * 120u32.into();
                let rent_fee = Perbill::from_rational_approximation(
                    rent_duration,
                    rent_order.rent_end - rent_order.rent_start,
                ) * rent_order.stake_amount;

                Self::pay_rent_fee(&rent_order, rent_fee, machine_id.clone())?;

                RentOrder::<T>::remove(rent_id);
            }
            MachineRentOrder::<T>::remove(&machine_id);

            // 解压机器质押的币
            <T as Config>::Currency::unreserve(
                &machine_info.machine_stash,
                machine_info.stake_amount,
            );

            MachinesInfo::<T>::remove(&machine_id);

            let machine_rent_order = Self::machine_rent_order(&machine_id);

            let mut stash_machines = Self::stash_machines(&machine_info.machine_stash);
            stash_machines.machine_exit(
                machine_id.clone(),
                machine_info.calc_point(),
                machine_info.gpu_num() as u64,
                // TODO: 注意，当机器被租用时(未经过confirm前)，需要同时增加stash_machine.
                // total_rented_gpu 和machine_rent_order.used_gpu
                machine_rent_order.used_gpu.len() as u64,
            );
            StashMachines::<T>::insert(&machine_info.machine_stash, stash_machines);

            LiveMachines::<T>::mutate(|live_machines| {
                live_machines.machine_exit(&machine_id);
            });

            MachineRentOrder::<T>::remove(machine_id);

            Ok(().into())
        }

        // 如果机器是在线状态，但是无法使用，可以举报。
        // 举报成功，100％没收质押币。50%举报人, 30%验证人, 20％国库
        #[pallet::weight(10000)]
        pub fn report_machine_fault(
            origin: OriginFor<T>,
            report_reason: IRMachineFaultType,
        ) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;

            let mut live_report = Self::live_report();
            let mut reporter_report = Self::reporter_report(&reporter);

            Self::pay_stake_when_report(reporter.clone())?;

            Self::do_report_machine_fault(
                reporter.clone(),
                report_reason,
                None,
                &mut live_report,
                &mut reporter_report,
            )?;
            LiveReport::<T>::put(live_report);
            ReporterReport::<T>::insert(&reporter, reporter_report);

            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn reporter_add_stake(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            Self::change_reporter_stake(reporter, amount, true)
        }

        #[pallet::weight(10000)]
        pub fn reporter_reduce_stake(
            origin: OriginFor<T>,
            amount: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            Self::change_reporter_stake(reporter, amount, false)
        }

        // 报告人可以在抢单之前取消该报告
        #[pallet::weight(10000)]
        pub fn reporter_cancel_report(
            origin: OriginFor<T>,
            report_id: ReportId,
        ) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            let report_info = Self::report_info(&report_id);

            ensure!(report_info.reporter == reporter, Error::<T>::NotReporter);
            ensure!(
                report_info.report_status == IRReportStatus::Reported,
                Error::<T>::ReportNotAllowCancel
            );

            ReporterStake::<T>::mutate(&reporter, |reporter_stake| {
                reporter_stake.change_stake_on_report_close(report_info.reporter_stake, false);
            });
            LiveReport::<T>::mutate(|live_report| {
                live_report.cancel_report(&report_id);
            });
            ReporterReport::<T>::mutate(&reporter, |reporter_report| {
                reporter_report.cancel_report(report_id);
            });
            ReportInfo::<T>::remove(&report_id);

            Self::deposit_event(Event::ReportCanceled(
                reporter,
                report_id,
                report_info.machine_fault_type,
            ));
            Ok(().into())
        }

        /// 委员会进行抢单
        #[pallet::weight(10000)]
        pub fn committee_book_report(
            origin: OriginFor<T>,
            report_id: ReportId,
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            Self::is_valid_committee(&committee)?;

            let mut report_info = Self::report_info(report_id);
            // 检查订单是否可以抢定
            report_info.can_book(&committee).map_err::<Error<T>, _>(Into::into)?;
            let order_stake = Self::get_stake_per_order()?;

            // 支付手续费或押金: 10 DBC | 1000 DBC
            // if let IRMachineFaultType::RentedInaccessible(..) = report_info.machine_fault_type {
            //     Self::pay_fixed_tx_fee(committee.clone())?;
            // } else {
            <T as Config>::ManageCommittee::change_used_stake(committee.clone(), order_stake, true)
                .map_err(|_| Error::<T>::StakeFailed)?;
            // }

            Self::book_report(committee.clone(), report_id, &mut report_info, order_stake);
            Self::deposit_event(Event::CommitteeBookReport(committee, report_id));
            Ok(().into())
        }

        // 报告人在委员会完成抢单后，30分钟内用委员会的公钥，提交加密后的故障信息
        // 只有报告机器故障或者无法租用时需要提交加密信息
        #[pallet::weight(10000)]
        pub fn reporter_add_encrypted_error_info(
            origin: OriginFor<T>,
            report_id: ReportId,
            to_committee: T::AccountId,
            encrypted_err_info: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let reporter = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut report_info = Self::report_info(&report_id);
            let mut committee_ops = Self::committee_report_ops(&to_committee, &report_id);

            // 检查报告可以提供加密信息
            // 该orde处于验证中, 且还没有提交过加密信息
            report_info
                .can_submit_encrypted_info(&reporter, &to_committee)
                .map_err::<Error<T>, _>(Into::into)?;
            ensure!(
                committee_ops.order_status == IRReportOrderStatus::WaitingEncrypt,
                Error::<T>::OrderStatusNotFeat
            );

            // report_info中插入已经收到了加密信息的委员会
            ItemList::add_item(&mut report_info.get_encrypted_info_committee, to_committee.clone());
            ReportInfo::<T>::insert(&report_id, report_info);

            committee_ops.add_encry_info(encrypted_err_info, now);
            CommitteeReportOps::<T>::insert(&to_committee, &report_id, committee_ops);

            Self::deposit_event(Event::EncryptedInfoSent(reporter, to_committee, report_id));
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        ControllerStashBonded(T::AccountId, T::AccountId),
        ServerRoomGenerated(T::AccountId, H256),
        StakeAdded(T::AccountId, BalanceOf<T>),
        StakeReduced(T::AccountId, BalanceOf<T>),
        MachineInfoAdded(MachineId),

        AddConfirmHash(T::AccountId, [u8; 16]),
        AddConfirmRaw(T::AccountId, MachineId),
        MachineDistributed(MachineId, T::AccountId),

        // Last item is rent order gpu_num
        RentBlockNum(RentOrderId, T::AccountId, MachineId, BalanceOf<T>, T::BlockNumber, u32),
        ConfirmReletBlockNum(T::AccountId, MachineId, BalanceOf<T>, T::BlockNumber),
        // Last item is rent order gpu_num
        ReletBlockNum(RentOrderId, T::AccountId, MachineId, BalanceOf<T>, T::BlockNumber, u32),

        ReportMachineFault(T::AccountId, IRMachineFaultType),
        ReporterAddStake(T::AccountId, BalanceOf<T>),
        ReporterReduceStake(T::AccountId, BalanceOf<T>),
        ReportCanceled(T::AccountId, ReportId, IRMachineFaultType),
        CommitteeBookReport(T::AccountId, ReportId),
        EncryptedInfoSent(T::AccountId, T::AccountId, ReportId),
    }

    #[pallet::error]
    pub enum Error<T> {
        AlreadyController,
        NoStashBond,
        PayTxFeeFailed,
        MachineIdExist,
        SigMachineIdNotEqualBondedMachineId,
        ConvertMachineIdToWalletFailed,
        BadSignature,
        BadMsgLen,
        MachineStashNotEqualControllerStash,
        BalanceNotEnough,
        TelecomIsNull,
        ServerRoomNotFound,
        NotAllowedChangeMachineInfo,
        NotMachineController,
        DuplicateHash,
        InfoNotFeatHash,
        NotInBookList,
        AlreadySubmitRaw,
        AlreadySubmitHash,
        NotSubmitHash,
        TimeNotAllow,
        MachineNotRentable,
        GetMachinePriceFailed,
        GPUNotEnough,
        OnlyHalfHourAllowed,
        OnlyAllowIntegerMultipleOfHour,
        Overflow,
        InsufficientValue,
        NoOrderExist,
        ExpiredConfirm,
        StatusNotAllowed,
        UnlockToPayFeeFailed,

        StakeNotEnough,
        ReportNotAllowCancel,
        ReportNotAllowBook,
        NotReporter,
        NotCommittee,
        AlreadyBooked,
        GetStakeAmountFailed,
        StakeFailed,
        OrderStatusNotFeat,
        NotOrderReporter,
        NotOrderCommittee,
    }
}

// 检查bonding信息
// TODO: 与online_profile合并
impl<T: Config> Pallet<T> {
    pub fn check_bonding_msg(
        stash: T::AccountId,
        machine_id: MachineId,
        msg: Vec<u8>,
        sig: Vec<u8>,
    ) -> DispatchResultWithPostInfo {
        // 验证msg: len(machine_id + stash_account) = 64 + 48
        ensure!(msg.len() == 112, Error::<T>::BadMsgLen);

        let (sig_machine_id, sig_stash_account) = (msg[..64].to_vec(), msg[64..].to_vec());
        ensure!(machine_id == sig_machine_id, Error::<T>::SigMachineIdNotEqualBondedMachineId);
        let sig_stash_account = Self::get_account_from_str(&sig_stash_account)
            .ok_or(Error::<T>::ConvertMachineIdToWalletFailed)?;
        ensure!(sig_stash_account == stash, Error::<T>::MachineStashNotEqualControllerStash);

        // 验证签名是否为MachineId发出
        ensure!(
            dbc_support::utils::verify_sig(msg, sig, machine_id).is_some(),
            Error::<T>::BadSignature
        );
        Ok(().into())
    }

    pub fn get_account_from_str(addr: &[u8]) -> Option<T::AccountId> {
        let account_id32: [u8; 32] = dbc_support::utils::get_accountid32(addr)?;
        T::AccountId::decode(&mut &account_id32[..]).ok()
    }
}

impl<T: Config> Pallet<T> {
    fn pay_fixed_tx_fee(who: T::AccountId) -> DispatchResultWithPostInfo {
        <generic_func::Module<T>>::pay_fixed_tx_fee(who).map_err(|_| Error::<T>::PayTxFeeFailed)?;
        Ok(().into())
    }

    // - Write: StashStake, Balance
    fn change_stash_total_stake(
        who: T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
    ) -> Result<(), ()> {
        let mut stash_stake = Self::stash_stake(&who);

        // 更改 stash_stake
        if is_add {
            stash_stake = stash_stake.checked_add(&amount).ok_or(())?;
            ensure!(<T as Config>::Currency::can_reserve(&who, amount), ());
            <T as Config>::Currency::reserve(&who, amount).map_err(|_| ())?;
        } else {
            stash_stake = stash_stake.checked_sub(&amount).ok_or(())?;
            <T as Config>::Currency::unreserve(&who, amount);
        }

        StashStake::<T>::insert(&who, stash_stake);

        if is_add {
            Self::deposit_event(Event::StakeAdded(who, amount));
        } else {
            Self::deposit_event(Event::StakeReduced(who, amount));
        }
        Ok(())
    }
}

impl<T: Config> Pallet<T> {
    // 获取所有新加入的机器，并进行分派给委员会
    pub fn distribute_machines() {
        let live_machines = Self::live_machines();
        let now = <frame_system::Module<T>>::block_number();
        let confirm_start = now + SUBMIT_HASH_END.into();

        for machine_id in live_machines.confirmed_machine {
            // 重新分配: 必须清空该状态
            if MachineCommittee::<T>::contains_key(&machine_id) {
                MachineCommittee::<T>::remove(&machine_id);
            }

            if let Some(committee_work_index) = Self::get_work_index() {
                for work_index in committee_work_index {
                    let _ = Self::book_one(machine_id.to_vec(), confirm_start, now, work_index);
                }
                // 将机器状态从ocw_confirmed_machine改为booked_machine
                Self::book_machine(machine_id.clone());
            };
        }
    }

    // 分派一个machineId给随机的委员会
    // 返回3个随机顺序的账户及其对应的验证顺序
    pub fn get_work_index() -> Option<Vec<VerifySequence<T::AccountId>>> {
        let mut committee = <committee::Module<T>>::available_committee()?;
        if committee.len() < 3 {
            return None
        };

        let mut verify_sequence = Vec::new();
        for i in 0..3 {
            let lucky_index =
                <generic_func::Module<T>>::random_u32(committee.len() as u32 - 1u32) as usize;
            verify_sequence.push(VerifySequence {
                who: committee[lucky_index].clone(),
                index: (i..DISTRIBUTION as usize).step_by(3).collect(),
            });
            committee.remove(lucky_index);
        }
        Some(verify_sequence)
    }

    // 一个委员会进行操作
    // - Writes: MachineCommittee, CommitteeMachine, CommitteeOps
    fn book_one(
        machine_id: MachineId,
        confirm_start: T::BlockNumber,
        now: T::BlockNumber,
        work_index: VerifySequence<T::AccountId>,
    ) -> Result<(), ()> {
        let stake_need = <T as Config>::ManageCommittee::stake_per_order().ok_or(())?;
        // Change committee usedstake will nerver fail after set proper params
        <T as Config>::ManageCommittee::change_used_stake(
            work_index.who.clone(),
            stake_need,
            true,
        )?;

        // 修改machine对应的委员会
        MachineCommittee::<T>::mutate(&machine_id, |machine_committee| {
            ItemList::add_item(&mut machine_committee.booked_committee, work_index.who.clone());
            machine_committee.book_time = now;
            machine_committee.confirm_start_time = confirm_start;
        });
        CommitteeMachine::<T>::mutate(&work_index.who, |committee_machine| {
            ItemList::add_item(&mut committee_machine.booked_machine, machine_id.clone());
        });
        CommitteeOnlineOps::<T>::mutate(&work_index.who, &machine_id, |committee_ops| {
            let start_time: Vec<_> = work_index
                .index
                .clone()
                .into_iter()
                .map(|x| now + (x as u32 * SUBMIT_RAW_START / DISTRIBUTION).into())
                .collect();

            committee_ops.staked_dbc = stake_need;
            committee_ops.verify_time = start_time;
            committee_ops.machine_status = IRVerifyMachineStatus::Booked;
        });

        Self::deposit_event(Event::MachineDistributed(machine_id.to_vec(), work_index.who));
        Ok(())
    }

    // - Write: LiveMachines, MachinesInfo
    fn book_machine(id: MachineId) {
        LiveMachines::<T>::mutate(|live_machines| {
            ItemList::rm_item(&mut live_machines.confirmed_machine, &id);
            ItemList::add_item(&mut live_machines.booked_machine, id.clone());
        });
        MachinesInfo::<T>::mutate(&id, |machine_info| {
            machine_info.machine_status = MachineStatus::CommitteeVerifying;
        });
    }

    fn statistic_online_verify() {
        let now = <frame_system::Module<T>>::block_number();
        let booked_machine = Self::live_machines().booked_machine;

        let committee_stake_per_order =
            <T as Config>::ManageCommittee::stake_per_order().unwrap_or_default();

        for machine_id in booked_machine {
            Self::summary_raw(machine_id, now, committee_stake_per_order);
        }
    }

    // 对已经提交完原始值的机器进行处理
    // - Writes: MachineCommittee, CommitteeMachine, CommitteeStake
    // CommitteeOps, MachineSubmitedHash, CommitteeMachine
    fn summary_raw(machine_id: MachineId, now: T::BlockNumber, stake_per_order: BalanceOf<T>) {
        let mut machine_committee = Self::machine_committee(&machine_id);

        // 如果是在提交Hash的状态，且已经到提交原始值的时间，则改变状态并返回
        if matches!(machine_committee.status, IRVerifyStatus::SubmittingHash) {
            if now >= machine_committee.book_time + SUBMIT_RAW_START.into() {
                machine_committee.status = IRVerifyStatus::SubmittingRaw;
                MachineCommittee::<T>::insert(&machine_id, machine_committee);
                return
            }
        }

        if !machine_committee.can_summary(now) {
            return
        }

        let summary_result = Self::summary_confirmation(&machine_id);
        let (inconsistent, unruly, reward) = summary_result.clone().get_committee_group();

        let mut stash_slash_info = None;

        match summary_result.clone() {
            IRMachineConfirmStatus::Confirmed(summary) => {
                if Self::confirm_machine(summary.valid_support.clone(), summary.info.unwrap())
                    .is_ok()
                {
                    // 如果机器成功上线，则从委员会确认的机器中删除，添加到成功上线的记录中
                    summary.valid_support.iter().for_each(|committee| {
                        CommitteeMachine::<T>::mutate(&committee, |machines| {
                            ItemList::add_item(&mut machines.online_machine, machine_id.clone());
                        });
                    });
                }
            },
            IRMachineConfirmStatus::Refuse(_summary) => {
                // should cancel machine_stash slash when slashed committee apply review
                stash_slash_info = Self::refuse_machine(machine_id.clone());
            },
            IRMachineConfirmStatus::NoConsensus(_summary) => {
                let _ = Self::revert_book(machine_id.clone());
                Self::revert_booked_machine(machine_id.clone());
            },
        }

        MachineCommittee::<T>::mutate(&machine_id, |machine_committee| {
            machine_committee.after_summary(summary_result.clone())
        });

        let is_refused = summary_result.is_refused();
        if inconsistent.is_empty() && unruly.is_empty() && !is_refused {
            // 没有惩罚时则直接退还委员会的质押
            for a_committee in reward {
                let _ = <T as Config>::ManageCommittee::change_used_stake(
                    a_committee,
                    stake_per_order,
                    false,
                );
            }
        } else {
            // 添加惩罚
            let slash_id = Self::get_new_slash_id();
            let (machine_stash, stash_slash_amount) = stash_slash_info.unwrap_or_default();
            PendingOnlineSlash::<T>::insert(
                slash_id,
                IRPendingOnlineSlashInfo {
                    machine_id: machine_id.clone(),
                    machine_stash,
                    stash_slash_amount,

                    inconsistent_committee: inconsistent,
                    unruly_committee: unruly,
                    reward_committee: reward,
                    committee_stake: stake_per_order,

                    slash_time: now,
                    slash_exec_time: now + TWO_DAY.into(),

                    book_result: summary_result.into_book_result(),
                    slash_result: IROnlineSlashResult::Pending,
                },
            );

            UnhandledOnlineSlash::<T>::mutate(|unhandled_slash| {
                ItemList::add_item(unhandled_slash, slash_id);
            });
        }

        // Do cleaning
        machine_committee.booked_committee.iter().for_each(|a_committee| {
            CommitteeOnlineOps::<T>::remove(&a_committee, &machine_id);
            MachineSubmitedHash::<T>::remove(&machine_id);
            CommitteeMachine::<T>::mutate(&a_committee, |committee_machine| {
                committee_machine.online_cleanup(&machine_id)
            });
        })
    }

    // 总结机器的确认情况: 检查机器是否被确认，并检查提交的信息是否一致
    // 返回三种状态：
    // 1. 无共识：处理办法：退还委员会质押，机器重新派单。
    // 2. 支持上线: 处理办法：扣除所有反对上线，支持上线但提交无效信息的委员会的质押。
    // 3. 反对上线: 处理办法：反对的委员会平分支持的委员会的质押。扣5%矿工质押，
    // 允许矿工再次质押而上线。
    pub fn summary_confirmation(machine_id: &MachineId) -> IRMachineConfirmStatus<T::AccountId> {
        let machine_committee = Self::machine_committee(machine_id);

        let mut summary = IRSummary::default();
        // 支持的委员会可能提交不同的机器信息
        let mut uniq_machine_info: Vec<CommitteeUploadInfo> = Vec::new();
        // 不同机器信息对应的委员会
        let mut committee_for_machine_info = Vec::new();

        // 记录没有提交原始信息的委员会
        summary.unruly = machine_committee.unruly_committee();

        // 如果没有人提交确认信息，则无共识。返回分派了订单的委员会列表，对其进行惩罚
        if machine_committee.confirmed_committee.is_empty() {
            return IRMachineConfirmStatus::NoConsensus(summary)
        }

        // 记录上反对上线的委员会
        for a_committee in machine_committee.confirmed_committee {
            let submit_machine_info =
                Self::committee_online_ops(a_committee.clone(), machine_id).machine_info;
            if !submit_machine_info.is_support {
                ItemList::add_item(&mut summary.against, a_committee);
            } else {
                match uniq_machine_info.iter().position(|r| r == &submit_machine_info) {
                    None => {
                        uniq_machine_info.push(submit_machine_info.clone());
                        committee_for_machine_info.push(vec![a_committee.clone()]);
                    },
                    Some(index) =>
                        ItemList::add_item(&mut committee_for_machine_info[index], a_committee),
                };
            }
        }

        // 统计committee_for_machine_info中有多少委员会站队最多
        let support_committee_num: Vec<usize> =
            committee_for_machine_info.iter().map(|item| item.len()).collect();
        // 最多多少个委员会达成一致意见
        let max_support = support_committee_num.clone().into_iter().max();
        if max_support.is_none() {
            // 如果没有支持者，且有反对者，则拒绝接入。
            if !summary.against.is_empty() {
                return IRMachineConfirmStatus::Refuse(summary)
            }
            // 反对者支持者都为0
            return IRMachineConfirmStatus::NoConsensus(summary)
        }

        let max_support_num = max_support.unwrap();

        // 多少个机器信息的支持等于最大的支持
        let max_support_group = support_committee_num
            .clone()
            .into_iter()
            .filter(|n| n == &max_support_num)
            .count();

        if max_support_group == 1 {
            let committee_group_index =
                support_committee_num.into_iter().position(|r| r == max_support_num).unwrap();

            // 记录所有的无效支持
            for (index, committees) in committee_for_machine_info.iter().enumerate() {
                if index != committee_group_index {
                    for a_committee in committees {
                        ItemList::add_item(&mut summary.invalid_support, a_committee.clone());
                    }
                }
            }

            if summary.against.len() > max_support_num {
                // 反对多于支持
                for a_committee in committee_for_machine_info[committee_group_index].clone() {
                    ItemList::add_item(&mut summary.invalid_support, a_committee);
                }
                return IRMachineConfirmStatus::Refuse(summary)
            } else if summary.against.len() == max_support_num {
                // 反对等于支持
                for a_committee in committee_for_machine_info[committee_group_index].clone() {
                    ItemList::add_item(&mut summary.invalid_support, a_committee);
                }
                summary.invalid_support = committee_for_machine_info[committee_group_index].clone();
                return IRMachineConfirmStatus::NoConsensus(summary)
            } else {
                // 反对小于支持
                // 记录上所有的有效支持
                summary.valid_support = committee_for_machine_info[committee_group_index].clone();
                summary.info = Some(uniq_machine_info[committee_group_index].clone());
                return IRMachineConfirmStatus::Confirmed(summary)
            }
        } else {
            // 如果多于两组是Max个委员会支, 则所有的支持都是无效的支持
            for committees in &committee_for_machine_info {
                for a_committee in committees {
                    ItemList::add_item(&mut summary.invalid_support, a_committee.clone());
                }
            }
            // Now will be Refuse or NoConsensus
            if summary.against.len() > max_support_num {
                return IRMachineConfirmStatus::Refuse(summary)
            } else {
                // against <= max_support 且 max_support_group > 1，且反对的不占多数
                return IRMachineConfirmStatus::NoConsensus(summary)
            }
        }
    }

    // - Writes: StashTotalStake, MachinesInfo, LiveMachines, StashMachines
    fn confirm_machine(
        reported_committee: Vec<T::AccountId>,
        committee_upload_info: CommitteeUploadInfo,
    ) -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let machine_id = committee_upload_info.machine_id.clone();
        let mut machine_info = Self::machines_info(&machine_id);

        // 解锁并退还用户的保证金
        Self::change_stash_total_stake(
            machine_info.machine_stash.clone(),
            machine_info.stake_amount,
            false,
        )?;

        machine_info.machine_online(now, committee_upload_info);
        machine_info.reward_committee = reported_committee;

        MachinesInfo::<T>::insert(&machine_id, machine_info.clone());
        LiveMachines::<T>::mutate(|live_machines| {
            ItemList::rm_item(&mut live_machines.booked_machine, &machine_id);
            ItemList::add_item(&mut live_machines.online_machine, machine_id.clone());
        });
        StashMachines::<T>::mutate(&machine_info.machine_stash, |stash_machine| {
            stash_machine.machine_online(
                machine_id,
                machine_info.gpu_num(),
                machine_info.calc_point(),
            )
        });
        Ok(())
    }

    // when committees reach an agreement to refuse machine, change machine status and record refuse
    // time
    fn refuse_machine(machine_id: MachineId) -> Option<(T::AccountId, BalanceOf<T>)> {
        // Refuse controller bond machine, and clean storage
        let machine_info = Self::machines_info(&machine_id);

        // Slash 100% of init stake(5% of one gpu stake)
        // 全部惩罚到国库
        let slash = machine_info.stake_amount;

        LiveMachines::<T>::mutate(|live_machines| live_machines.refuse_machine(machine_id.clone()));
        MachinesInfo::<T>::remove(&machine_id);
        StashMachines::<T>::mutate(&machine_info.machine_stash, |stash_machines| {
            stash_machines.refuse_machine(&machine_id);
        });

        Some((machine_info.machine_stash, slash))
    }

    // 重新进行派单评估
    // 该函数将清除本模块信息，并将online_profile机器状态改为ocw_confirmed_machine
    // 清除信息： IRCommitteeMachineList, IRMachineCommitteeList, IRCommitteeOps
    fn revert_book(machine_id: MachineId) -> Result<(), ()> {
        let machine_committee = Self::machine_committee(&machine_id);

        // 清除预订了机器的委员会
        for booked_committee in machine_committee.booked_committee {
            CommitteeOnlineOps::<T>::remove(&booked_committee, &machine_id);
            CommitteeMachine::<T>::mutate(&booked_committee, |committee_machine| {
                committee_machine.revert_book(&machine_id)
            })
        }

        MachineCommittee::<T>::remove(&machine_id);
        Ok(())
    }

    // 由于委员会没有达成一致，需要重新返回到bonding_machine
    fn revert_booked_machine(id: MachineId) {
        LiveMachines::<T>::mutate(|live_machines| live_machines.revert_book(id.clone()));
        MachinesInfo::<T>::mutate(&id, |machine_info| machine_info.revert_book())
    }
}

impl<T: Config> Pallet<T> {
    /// 根据GPU数量和该机器算力点数，计算该机器相比标准配置的租用价格
    // standard_point / machine_point ==  standard_price / machine_price
    // =>
    // machine_price = standard_price * machine_point / standard_point
    fn get_machine_price(machine_point: u64, need_gpu: u32, total_gpu: u32) -> Option<u64> {
        if total_gpu == 0 {
            return None
        }
        let standard_gpu_point_price = Self::standard_gpu_point_price()?;
        standard_gpu_point_price
            .gpu_price
            .checked_mul(machine_point)?
            .checked_mul(10_000)?
            .checked_div(standard_gpu_point_price.gpu_point)?
            .checked_mul(need_gpu as u64)?
            .checked_div(total_gpu as u64)?
            .checked_div(10_000)
    }

    // - Write: RenterTotalStake
    fn change_renter_total_stake(
        who: &T::AccountId,
        amount: BalanceOf<T>,
        is_add: bool,
    ) -> Result<(), ()> {
        let current_stake = Self::renter_total_stake(who);

        let new_stake = if is_add {
            ensure!(<T as Config>::Currency::can_reserve(who, amount), ());
            <T as Config>::Currency::reserve(who, amount).map_err(|_| ())?;
            current_stake.checked_add(&amount).ok_or(())?
        } else {
            ensure!(current_stake >= amount, ());
            let _ = <T as Config>::Currency::unreserve(who, amount);
            current_stake.checked_sub(&amount).ok_or(())?
        };
        RenterTotalStake::<T>::insert(who, new_stake);
        Ok(())
    }

    // 获取一个新的租用订单的ID
    pub fn get_new_rent_id() -> RentOrderId {
        let rent_id = Self::next_rent_id();

        let new_rent_id = loop {
            let new_rent_id = if rent_id == u64::MAX { 0 } else { rent_id + 1 };
            if !RentOrder::<T>::contains_key(new_rent_id) {
                break new_rent_id
            }
        };

        NextRentId::<T>::put(new_rent_id);

        rent_id
    }

    // 在rent_machine; rent_machine_by_minutes中使用, confirm_rent之前
    fn change_machine_status_on_rent_start(machine_id: &MachineId, gpu_num: u32) {
        MachinesInfo::<T>::mutate(machine_id, |machine_info| {
            machine_info.machine_status = MachineStatus::Rented;
        });
        MachineRentedGPU::<T>::mutate(machine_id, |machine_rented_gpu| {
            *machine_rented_gpu = machine_rented_gpu.saturating_add(gpu_num);
        });
    }

    // 在confirm_rent中使用
    // - Writes: LiveMachine, MachineInfo, StashMachine
    fn change_machine_status_on_confirmed(machine_id: &MachineId, renter: T::AccountId) {
        MachinesInfo::<T>::mutate(machine_id, |machine_info| {
            StashMachines::<T>::mutate(&machine_info.machine_stash, |stash_machine| {
                stash_machine.total_rented_gpu =
                    stash_machine.total_rented_gpu.saturating_add(machine_info.gpu_num() as u64);
            });

            ItemList::add_item(&mut machine_info.renters, renter);
            machine_info.total_rented_times += 1;
        });

        LiveMachines::<T>::mutate(|live_machines| {
            ItemList::rm_item(&mut live_machines.online_machine, machine_id);
            ItemList::add_item(&mut live_machines.rented_machine, machine_id.clone());
        });
    }

    // 当租用结束，或者租用被终止时，将保留的金额支付给stash账户，剩余部分解锁给租用人
    // NOTE: 租金的1%将分给验证人
    fn pay_rent_fee(
        rent_order: &RentOrderDetail<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        mut rent_fee: BalanceOf<T>,
        machine_id: MachineId,
    ) -> DispatchResult {
        let mut machine_info = Self::machines_info(&machine_id);

        <T as Config>::Currency::unreserve(&rent_order.renter, rent_fee);

        // NOTE: 将租金的1%转给委员会，剩余的转给stash账户
        // 可能足用人质押数量大于需要支付的租金，因此需要解绑质押，再转对应的租金
        let reward_to_stash = Perbill::from_rational_approximation(99u32, 100u32) * rent_fee;
        let reward_to_committee = rent_fee.saturating_sub(reward_to_stash);
        let committee_each_get = Perbill::from_rational_approximation(
            1u32,
            machine_info.reward_committee.len() as u32,
        ) * reward_to_committee;
        for a_committee in machine_info.reward_committee.clone() {
            let _ = <T as Config>::Currency::transfer(
                &rent_order.renter,
                &a_committee,
                committee_each_get,
                KeepAlive,
            );
            rent_fee = rent_fee.saturating_sub(committee_each_get);
        }
        let _ = <T as Config>::Currency::transfer(
            &rent_order.renter,
            &machine_info.machine_stash,
            rent_fee,
            KeepAlive,
        );

        // 根据机器GPU计算需要多少质押
        let max_stake = Self::stake_per_gpu()
            .checked_mul(&machine_info.gpu_num().saturated_into::<BalanceOf<T>>())
            .ok_or(Error::<T>::Overflow)?;
        if max_stake > machine_info.stake_amount {
            // 如果 rent_fee >= max_stake - machine_info.stake_amount,
            // 则质押 max_stake - machine_info.stake_amount
            // 如果 rent_fee < max_stake - machine_info.stake_amount, 则质押 rent_fee
            let stake_amount = rent_fee.min(max_stake - machine_info.stake_amount);

            <T as Config>::Currency::reserve(&machine_info.machine_stash, stake_amount)?;
            machine_info.stake_amount = machine_info.stake_amount.saturating_add(stake_amount);
            MachinesInfo::<T>::insert(&machine_id, machine_info);
        }

        Ok(())
    }

    // 这里修rentMachine模块通知onlineProfile机器已经租用完成，
    // onlineProfile判断机器是否需要变成online状态，或者记录下之前是租用状态，
    // 以便机器再次上线时进行正确的惩罚
    fn check_if_rent_finished() {
        let now = <frame_system::Module<T>>::block_number();
        if !<PendingRentEnding<T>>::contains_key(now) {
            return
        }
        let pending_ending = Self::pending_rent_ending(now);

        for rent_id in pending_ending {
            let rent_order = Self::rent_order(&rent_id);
            let machine_id = rent_order.machine_id.clone();
            let rent_duration = now - rent_order.rent_start;

            let _ = Self::pay_rent_fee(&rent_order, rent_order.stake_amount, machine_id.clone());

            // NOTE: 只要机器还有租用订单(租用订单>1)，就不修改成online状态。
            let is_last_rent = Self::is_last_rent(&machine_id);
            Self::change_machine_status_on_rent_end(
                &machine_id,
                rent_order.gpu_num,
                rent_duration,
                is_last_rent,
                rent_order.renter.clone(),
            );

            Self::clean_order(&rent_order.renter, rent_id);
        }
    }

    // - Writes: MachineRentedGPU, LiveMachines, MachinesInfo, StashMachine
    fn change_machine_status_on_rent_end(
        machine_id: &MachineId,
        rented_gpu_num: u32,
        rent_duration: T::BlockNumber,
        is_last_rent: bool,
        renter: T::AccountId,
    ) {
        let mut machine_info = Self::machines_info(machine_id);
        let mut live_machines = Self::live_machines();

        // 租用结束
        let gpu_num = machine_info.gpu_num();
        if gpu_num == 0 {
            return
        }
        machine_info.total_rented_duration +=
            Perbill::from_rational_approximation(rented_gpu_num, gpu_num) * rent_duration;
        ItemList::rm_item(&mut machine_info.renters, &renter);

        match machine_info.machine_status {
            MachineStatus::ReporterReportOffline(..) | MachineStatus::StakerReportOffline(..) => {
                RentedFinished::<T>::insert(machine_id, renter);
            },
            MachineStatus::Rented => {
                // machine_info.machine_status = new_status;

                // NOTE: 考虑是不是last_rent
                if is_last_rent {
                    ItemList::rm_item(&mut live_machines.rented_machine, machine_id);
                    ItemList::add_item(&mut live_machines.online_machine, machine_id.clone());

                    machine_info.last_online_height = <frame_system::Module<T>>::block_number();
                    machine_info.machine_status = MachineStatus::Online;

                    // 租用结束
                    StashMachines::<T>::mutate(&machine_info.machine_stash, |stash_machine| {
                        stash_machine.total_rented_gpu =
                            stash_machine.total_rented_gpu.saturating_sub(gpu_num.into());
                    });
                }
            },
            _ => {},
        }

        MachineRentedGPU::<T>::mutate(machine_id, |machine_rented_gpu| {
            *machine_rented_gpu = machine_rented_gpu.saturating_sub(rented_gpu_num);
        });
        LiveMachines::<T>::put(live_machines);
        MachinesInfo::<T>::insert(&machine_id, machine_info);
    }

    // -Write: MachineRentOrder, PendingRentEnding, RentOrder,
    // UserRented, PendingConfirming
    fn clean_order(who: &T::AccountId, rent_order_id: RentOrderId) {
        let rent_order = Self::rent_order(rent_order_id);

        let mut pending_rent_ending = Self::pending_rent_ending(rent_order.rent_end);
        ItemList::rm_item(&mut pending_rent_ending, &rent_order_id);
        if pending_rent_ending.is_empty() {
            PendingRentEnding::<T>::remove(rent_order.rent_end);
        } else {
            PendingRentEnding::<T>::insert(rent_order.rent_end, pending_rent_ending);
        }

        let pending_confirming_deadline = rent_order.rent_start + WAITING_CONFIRMING_DELAY.into();
        let mut pending_confirming = Self::pending_confirming(pending_confirming_deadline);
        ItemList::rm_item(&mut pending_confirming, &rent_order_id);
        if pending_confirming.is_empty() {
            PendingConfirming::<T>::remove(pending_confirming_deadline);
        } else {
            PendingConfirming::<T>::insert(pending_confirming_deadline, pending_confirming);
        }

        let mut machine_rent_order = Self::machine_rent_order(&rent_order.machine_id);
        machine_rent_order.clean_expired_order(rent_order_id, rent_order.gpu_index);
        MachineRentOrder::<T>::insert(&rent_order.machine_id, machine_rent_order);

        let mut rent_order_list = Self::user_rented(who);
        ItemList::rm_item(&mut rent_order_list, &rent_order_id);
        if rent_order_list.is_empty() {
            UserRented::<T>::remove(who);
        } else {
            UserRented::<T>::insert(who, rent_order_list);
        }

        RentOrder::<T>::remove(rent_order_id);
    }

    // 当没有正在租用的机器时，可以修改得分快照
    fn is_last_rent(machine_id: &MachineId) -> bool {
        let machine_order = Self::machine_rent_order(machine_id);
        let mut renting_count = 0;

        // NOTE: 一定是正在租用的机器才算，正在确认中的租用不算
        for order_id in machine_order.rent_order {
            let rent_order = Self::rent_order(order_id);
            if matches!(rent_order.rent_status, RentStatus::Renting) {
                renting_count += 1;
            }
        }

        renting_count < 2
    }

    fn check_if_offline_timeout() -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        if !<OfflineMachines<T>>::contains_key(now) {
            return Ok(())
        }
        let offline_machines = Self::offline_machines(now);

        for machine_id in offline_machines {
            let machine_info = Self::machines_info(&machine_id);
            if matches!(machine_info.machine_status, MachineStatus::StakerReportOffline(..)) {
                <T as Config>::SlashAndReward::slash_and_reward(
                    vec![machine_info.machine_stash.clone()],
                    machine_info.stake_amount,
                    vec![],
                )?;
            }
        }
        OfflineMachines::<T>::remove(now);
        Ok(())
    }
}

// For Slash
impl<T: Config> Pallet<T> {
    fn get_new_slash_id() -> u64 {
        let slash_id = Self::next_slash_id();

        if slash_id == u64::MAX {
            NextSlashId::<T>::put(0);
        } else {
            NextSlashId::<T>::put(slash_id + 1);
        };

        slash_id
    }
}
