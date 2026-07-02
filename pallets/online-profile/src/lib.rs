#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

// pub mod migrations;
mod online_reward;
mod rpc;
mod slash;
mod traits;
mod types;
mod utils;

pub mod migration;
use dbc_support::{
    live_machine::LiveMachine,
    machine_info::MachineInfo,
    machine_type::{Latitude, Longitude, MachineStatus, StakerCustomizeInfo},
    traits::{DbcPrice, GNOps, ManageCommittee, RentalStatus, RentTerminateOnOffline},
    verify_online::StashMachine,
    verify_slash::{OPPendingSlashInfo, OPPendingSlashReviewInfo, OPSlashReason},
    EraIndex, ItemList, MachineId, SlashId, ONE_DAY,
};
use frame_support::{
    dispatch::DispatchResultWithPostInfo,
    pallet_prelude::*,
    traits::{Currency, EnsureOrigin, Get, OnUnbalanced, ReservableCurrency},
    weights::Weight,
};
use frame_system::pallet_prelude::*;
use sp_core::H256;
use sp_runtime::{
    traits::{CheckedAdd, CheckedMul, CheckedSub, Saturating, Zero},
    Perbill, SaturatedConversion,
};
use sp_std::{
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    convert::From,
    prelude::*,
    str,
    vec::Vec,
};

pub use pallet::*;
pub use types::*;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

use frame_support::traits::{GetStorageVersion, StorageVersion};
// FRAME pallet storage version. Gates on_runtime_upgrade (bumped 0 -> 1 in
// spec 413 so rebuild_sys_info runs once, not every upgrade). Distinct from the
// legacy custom `StorageVersion<T>` u16 storage item below, which was set by
// historical migrations and is no longer read by live code.
const STORAGE_VERSION: StorageVersion = StorageVersion::new(1);
#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + generic_func::Config + pallet_timestamp::Config {
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type BondingDuration: Get<EraIndex>;
        type DbcPrice: DbcPrice<Balance = BalanceOf<Self>>;
        type ManageCommittee: ManageCommittee<
            AccountId = Self::AccountId,
            Balance = BalanceOf<Self>,
        >;
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
        type CancelSlashOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        type SlashAndReward: GNOps<AccountId = Self::AccountId, Balance = BalanceOf<Self>>;
        /// [+30% 桥·跨系统互斥] 查询另一套独立租用系统(terminating-rental)的租用状态，用于
        /// deeplink_set_rented 拒绝对已在 terminating-rental 租用的机器叠加 DeepLink 租（防跨系统 +30% double-count）。
        /// runtime 里接 TerminatingRental；mock 里可接 () 空实现。
        type TerminatingRentalStatus: dbc_support::traits::RentalStatus<MachineId = MachineId>;
        /// [Thread B ③ · 离线终止] 在租机器被健康检测器(DDN)/控制账户报离线时，通知 rent-machine
        /// 结算并终止该机所有在租订单（offline=true、罚≤24h 租金给租客、不碰 stake bond）。
        /// runtime 里接 RentMachine；不涉及原生租用的 mock 接 () 空实现。
        type RentTerminate: dbc_support::traits::RentTerminateOnOffline<MachineId = MachineId>;
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    #[pallet::storage_version(STORAGE_VERSION)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn online_stake_params)]
    pub(super) type OnlineStakeParams<T: Config> =
        StorageValue<_, OnlineStakeParamsInfo<BalanceOf<T>>>;

    /// A standard example for rent fee calculation(price: USD*10^6)
    #[pallet::storage]
    #[pallet::getter(fn standard_gpu_point_price)]
    pub(super) type StandardGPUPointPrice<T: Config> =
        StorageValue<_, dbc_support::machine_type::StandardGpuPointPrice>;

    /// Reonline to change hardware, should stake some balance
    #[pallet::storage]
    #[pallet::getter(fn user_mut_hardware_stake)]
    pub(super) type UserMutHardwareStake<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        MachineId,
        UserMutHardwareStakeInfo<BalanceOf<T>, T::BlockNumber>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn rent_fee_destroy_percent)]
    pub(super) type RentFeeDestroyPercent<T: Config> =
        StorageValue<_, Perbill, ValueQuery, RentFeeDestroyPercentDefault<T>>;

    #[pallet::type_value]
    pub(super) fn RentFeeDestroyPercentDefault<T: Config>() -> Perbill {
        Perbill::from_percent(5)
    }

    /// 卡主自定义额外加价（USD×10^6 per day per GPU），在系统自动定价基础上叠加
    /// 两个租赁 pallet (rent-machine, terminating-rental) 共享此 Storage
    #[pallet::storage]
    #[pallet::getter(fn machine_extra_price)]
    pub type MachineExtraPrice<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, u64, ValueQuery>;

    /// 机器租赁模式：FullTime | TimeSlot（默认 FullTime）
    #[pallet::storage]
    #[pallet::getter(fn machine_rental_mode)]
    pub type MachineRentalModeStorage<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, MachineRentalMode, ValueQuery>;

    /// 每周循环时段表：MachineId → 7 天 × Vec<TimeRange>（索引 0=周日, 1=周一, ..., 6=周六）
    /// UTC 时间
    #[pallet::storage]
    #[pallet::getter(fn weekly_schedule)]
    pub type WeeklySchedule<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, [Vec<TimeRange>; 7], ValueQuery>;

    /// 特定日期的时段表：优先级高于每周循环
    /// key2: 自 UNIX epoch (1970-01-01) 起的天数
    #[pallet::storage]
    #[pallet::getter(fn specific_date_schedule)]
    pub type SpecificDateSchedule<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        MachineId,
        Blake2_128Concat,
        u32,
        Vec<TimeRange>,
        ValueQuery,
    >;

    /// Statistics of gpu and stake
    #[pallet::storage]
    #[pallet::getter(fn sys_info)]
    pub type SysInfo<T: Config> = StorageValue<_, SysInfoDetail<BalanceOf<T>>, ValueQuery>;

    /// Statistics of gpu in one position
    #[pallet::storage]
    #[pallet::getter(fn pos_gpu_info)]
    pub type PosGPUInfo<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        Longitude,
        Blake2_128Concat,
        Latitude,
        PosInfo,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn stash_controller)]
    pub(super) type StashController<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    #[pallet::storage]
    #[pallet::getter(fn controller_stash)]
    pub(super) type ControllerStash<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    /// Detail info of machines
    #[pallet::storage]
    #[pallet::getter(fn machines_info)]
    pub type MachinesInfo<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
    >;

    /// 记录机器被租用的GPU个数
    #[pallet::storage]
    #[pallet::getter(fn machine_rented_gpu)]
    pub type MachineRentedGPU<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, u32, ValueQuery>;

    /// Statistics of stash account
    #[pallet::storage]
    #[pallet::getter(fn stash_machines)]
    pub(super) type StashMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, StashMachine<BalanceOf<T>>, ValueQuery>;

    /// Server rooms in stash account
    #[pallet::storage]
    #[pallet::getter(fn stash_server_rooms)]
    pub(super) type StashServerRooms<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Vec<H256>, ValueQuery>;

    /// All machines controlled by controller
    #[pallet::storage]
    #[pallet::getter(fn controller_machines)]
    pub(super) type ControllerMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Vec<MachineId>, ValueQuery>;

    /// 系统中存储有数据的机器
    #[pallet::storage]
    #[pallet::getter(fn live_machines)]
    pub type LiveMachines<T: Config> = StorageValue<_, LiveMachine, ValueQuery>;

    /// Block/Era
    #[pallet::storage]
    #[pallet::getter(fn current_era)]
    pub type CurrentEra<T: Config> = StorageValue<_, EraIndex, ValueQuery>;

    /// 每个Era机器的得分快照
    #[pallet::storage]
    #[pallet::getter(fn eras_stash_points)]
    pub(super) type ErasStashPoints<T: Config> =
        StorageMap<_, Blake2_128Concat, EraIndex, EraStashPoints<T::AccountId>, ValueQuery>;

    /// 每个Era机器的得分快照
    #[pallet::storage]
    #[pallet::getter(fn eras_machine_points)]
    pub(super) type ErasMachinePoints<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        EraIndex,
        BTreeMap<MachineId, MachineGradeStatus>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn phase_reward_info)]
    pub(super) type PhaseRewardInfo<T: Config> =
        StorageValue<_, PhaseRewardInfoDetail<BalanceOf<T>>>;

    #[pallet::storage]
    #[pallet::getter(fn era_reward)]
    pub(super) type EraReward<T: Config> =
        StorageMap<_, Blake2_128Concat, EraIndex, BalanceOf<T>, ValueQuery>;

    /// 某个Era机器获得的总奖励
    #[pallet::storage]
    #[pallet::getter(fn eras_machine_reward)]
    pub(super) type ErasMachineReward<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        EraIndex,
        Blake2_128Concat,
        MachineId,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// 某个Era机器释放的总奖励
    #[pallet::storage]
    #[pallet::getter(fn eras_machine_released_reward)]
    pub(super) type ErasMachineReleasedReward<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        EraIndex,
        Blake2_128Concat,
        MachineId,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// 某个Era Stash获得的总奖励
    #[pallet::storage]
    #[pallet::getter(fn eras_stash_reward)]
    pub(super) type ErasStashReward<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        EraIndex,
        Blake2_128Concat,
        T::AccountId,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// 某个Era Stash解锁的总奖励
    #[pallet::storage]
    #[pallet::getter(fn eras_stash_released_reward)]
    pub(super) type ErasStashReleasedReward<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        EraIndex,
        Blake2_128Concat,
        T::AccountId,
        BalanceOf<T>,
        ValueQuery,
    >;

    /// store max 150 era reward
    #[pallet::storage]
    #[pallet::getter(fn machine_recent_reward)]
    pub(super) type MachineRecentReward<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        MachineRecentRewardInfo<T::AccountId, BalanceOf<T>>,
    >;

    /// [+30% 桥] 机器是否被 DeepLink(EVM RentDBC) 租用。独立于原生 rent-machine 的 is_rented，
    /// 用于幂等守卫（防 deeplink_set_rented 重复 toggle 导致 total_rented_gpu/grade 会计重复加减）。
    #[pallet::storage]
    #[pallet::getter(fn deeplink_rented)]
    pub(super) type DeepLinkRented<T: Config> = StorageMap<_, Blake2_128Concat, MachineId, bool, ValueQuery>;

    /// 将要发放奖励的机器
    #[pallet::storage]
    #[pallet::getter(fn all_machine_id_snap)]
    pub(super) type AllMachineIdSnap<T: Config> =
        StorageValue<_, types::AllMachineIdSnapDetail, ValueQuery>;

    /// 资金账户的质押总计
    #[pallet::storage]
    #[pallet::getter(fn stash_stake)]
    pub type StashStake<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_slash_id)]
    pub(super) type NextSlashId<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pending_slash)]
    pub(super) type PendingSlash<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn pending_slash_review)]
    pub(super) type PendingSlashReview<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        SlashId,
        OPPendingSlashReviewInfo<T::AccountId, BalanceOf<T>, T::BlockNumber>,
    >;

    // 记录块高 -> 到期的slash_review
    #[pallet::storage]
    #[pallet::getter(fn pending_slash_review_checking)]
    pub(super) type PendingSlashReviewChecking<T: Config> =
        StorageMap<_, Blake2_128Concat, T::BlockNumber, Vec<SlashId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn rented_finished)]
    pub(super) type RentedFinished<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, T::AccountId>;

    // 记录某个时间需要执行的惩罚
    #[pallet::storage]
    #[pallet::getter(fn pending_exec_slash)]
    pub(super) type PendingExecSlash<T: Config> =
        StorageMap<_, Blake2_128Concat, T::BlockNumber, Vec<SlashId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn max_slash_execed)]
    pub(super) type MaxSlashExeced<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, T::BlockNumber, ValueQuery>;

    // The current storage version.
    #[pallet::storage]
    #[pallet::getter(fn storage_version)]
    pub(super) type StorageVersion<T: Config> = StorageValue<_, u16, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn offline_machine_to_renters)]
    pub(super) type OfflineMachine2renters<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, Vec<T::AccountId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn machine_to_pending_slash_ids)]
    pub(super) type Machine2PendingSlashIds<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, Vec<SlashId>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn authorized_force_exit_accounts)]
    pub(super) type AuthorizedForceExitAccounts<T: Config> =
        StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    /// [Thread B ① · DLC 化] 授权的离线检测器账户（复用 DeepLink DDN 的链上钱包）。
    /// 这些账户可对**任意机器**（闲置/被租/DeepLink 租）上报离线，不要求先租下机器——
    /// 对齐 DLC 的 DistributedDetectionNode→DBCAI.notify(MachineOffline) 模式（链下 5min 防抖+后端二次确认）。
    /// root 管理（set_offline_detectors）。空集时该路径不可用（fail-closed）。
    #[pallet::storage]
    #[pallet::getter(fn offline_detectors)]
    pub(super) type OfflineDetectors<T: Config> =
        StorageValue<_, Vec<T::AccountId>, ValueQuery>;

    /// Per-stash 自定义收租钱包（spec 410）
    /// 若不存在（矿工未配置），则默认租金走 stash 本账户。
    #[pallet::storage]
    #[pallet::getter(fn stash_rent_receiver)]
    pub(super) type StashRentReceiver<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(block_number: T::BlockNumber) -> Weight {
            Self::backup_and_reward(block_number);

            if block_number.saturated_into::<u64>() % (ONE_DAY as u64) == 1 {
                // Era开始时，生成当前Era和下一个Era的快照
                // 每天执行一次
                Self::update_snap_for_new_era();
            }
            Self::exec_pending_slash();
            let _ = Self::check_pending_slash();
            Weight::zero()
        }

        fn on_runtime_upgrade() -> Weight {
            // spec 411: rebuild SysInfo.total_gpu_num / total_rented_gpu from
            // MachinesInfo (source of truth). Repairs accounting drift produced
            // by the do_machine_exit bug fixed in that release. spec 413: gate it
            // behind the pallet storage version so it runs once (on the 0 -> 1
            // upgrade) instead of re-iterating all machines on every future
            // runtime upgrade. The rebuild is idempotent, so running it once more
            // here is harmless; the gate just stops the needless repeat work.
            let onchain = Pallet::<T>::on_chain_storage_version();
            if onchain >= 1 {
                return <T as frame_system::Config>::DbWeight::get().reads(1)
            }
            let w = crate::migration::rebuild_sys_info_from_machines_info::<T>();
            STORAGE_VERSION.put::<Pallet<T>>();
            w.saturating_add(<T as frame_system::Config>::DbWeight::get().reads_writes(1, 1))
        }

        // fn on_runtime_upgrade() -> Weight {
        //     let now = <frame_system::Pallet<T>>::block_number();

        //     let machine_id = "f2ed83f9fe6d26ae802fd26e021588ed3f33cd953dc5b208e627d69387336151"
        //         .as_bytes()
        //         .to_vec();

        //     let machine_info_result = Self::machines_info(&machine_id);
        //     match machine_info_result {
        //         Some(mut machine_info) => {
        //             if machine_info.reward_deadline > 0 {
        //                 return Weight::zero()
        //             }

        //             machine_info.online_height = now;
        //             let current_era = Self::current_era();
        //             machine_info.reward_deadline = current_era + REWARD_DURATION;

        //             MachineRecentReward::<T>::insert(
        //                 &machine_id,
        //                 MachineRecentRewardInfo {
        //                     machine_stash: machine_info.machine_stash.clone(),
        //                     reward_committee_deadline: machine_info.reward_deadline,
        //                     reward_committee: machine_info.reward_committee.clone(),
        //                     recent_machine_reward: VecDeque::new(),
        //                     recent_reward_sum: 0u32.into(),
        //                 },
        //             );

        //             machine_info.last_online_height = now;
        //             machine_info.last_machine_restake = now;
        //         },
        //         None => {},
        //     }

        //     Weight::zero()
        // }

        // fn on_runtime_upgrade() -> frame_support::weights::Weight {
        //     frame_support::log::info!("🔍 OnlineProfile storage upgrade start");
        //     if let Some(mut stake_params) = Self::online_stake_params() {
        //         stake_params.online_stake_usd_limit = 800000000;
        //         OnlineStakeParams::<T>::put(stake_params);
        //     }
        //     frame_support::log::info!("🚀 OnlineProfile storage upgrade end");
        //     Weight::zero()
        // }

        // From 800 USD -> 300 USD
        // fn on_runtime_upgrade() -> frame_support::weights::Weight {
        //     let mut online_stake_params = match Self::online_stake_params() {
        //         Some(params) => params,
        //         None => return Weight::zero(),
        //     };
        //     let online_stake_usd_limit =
        //         Perbill::from_rational(3u32, 8u32) * online_stake_params.online_stake_usd_limit;
        //     online_stake_params.online_stake_usd_limit = online_stake_usd_limit;
        //     OnlineStakeParams::<T>::put(online_stake_params);
        //     Weight::zero()
        // }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// When reward start to distribute
        #[pallet::call_index(0)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn set_reward_info(
            origin: OriginFor<T>,
            reward_info: PhaseRewardInfoDetail<BalanceOf<T>>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            <PhaseRewardInfo<T>>::put(reward_info);
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn set_online_stake_params(
            origin: OriginFor<T>,
            online_stake_params_info: OnlineStakeParamsInfo<BalanceOf<T>>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            OnlineStakeParams::<T>::put(online_stake_params_info);
            Ok(().into())
        }

        /// 设置标准GPU标准算力与租用价格
        #[pallet::call_index(2)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn set_standard_gpu_point_price(
            origin: OriginFor<T>,
            point_price: dbc_support::machine_type::StandardGpuPointPrice,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StandardGPUPointPrice::<T>::put(point_price);
            Ok(().into())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn set_rentfee_destroy_percent(
            origin: OriginFor<T>,
            percent: Perbill,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            RentFeeDestroyPercent::<T>::put(percent);
            Ok(().into())
        }

        /// Stash account set a controller
        #[pallet::call_index(4)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
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

        // - Writes: controller_machines, stash_controller, controller_stash, machine_info,
        /// Stash account reset controller for one machine
        #[pallet::call_index(5)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn stash_reset_controller(
            origin: OriginFor<T>,
            new_controller: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let stash = ensure_signed(origin)?;
            ensure!(
                !<ControllerStash<T>>::contains_key(&new_controller),
                Error::<T>::AlreadyController
            );

            let pre_controller = Self::stash_controller(&stash).ok_or(Error::<T>::Unknown)?;
            let controller_machines = Self::controller_machines(&pre_controller);

            controller_machines
                .iter()
                .try_for_each(|machine_id| -> Result<(), DispatchError> {
                    MachinesInfo::<T>::try_mutate(&machine_id, |machine_info| {
                        let machine_info = machine_info.as_mut().ok_or(Error::<T>::Unknown)?;
                        machine_info.controller = new_controller.clone();
                        Ok::<(), sp_runtime::DispatchError>(())
                    })?;
                    Ok(())
                })?;

            ControllerMachines::<T>::remove(&pre_controller);
            ControllerMachines::<T>::insert(&new_controller, controller_machines);

            StashController::<T>::insert(stash.clone(), new_controller.clone());
            ControllerStash::<T>::remove(pre_controller.clone());
            ControllerStash::<T>::insert(new_controller.clone(), stash.clone());

            Self::deposit_event(Event::StashResetController(stash, pre_controller, new_controller));
            Ok(().into())
        }

        /// Controller account reonline machine, allow change hardware info
        /// Committee will verify it later
        /// NOTE: User need to add machine basic info(pos & net speed), after
        /// committee verify finished, will be slashed for `OnlineReportOffline`
        #[pallet::call_index(6)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn offline_machine_change_hardware_info(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Pallet<T>>::block_number();
            let mut machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;

            ensure!(machine_info.is_controller(controller), Error::<T>::NotMachineController);
            // 只允许在线状态的机器修改信息
            ensure!(machine_info.is_online(), Error::<T>::MachineStatusNotAllowed);
            // [审计修 #5c — round2 MED] 拒绝对 DeepLink(EVM) 租用中的机器改硬件。
            //   这是第三条离线路径（machine_offline / controller_report_offline 之外），本函数直接把 is_online 机器
            //   置为 StakerReportOffline 且【不回退 +30% 租用快照】。若放行 DeepLink 租用机：
            //   (a) +30% 会被孤立在 era 快照 total 里（机器点数已移除、stash 统计可能被删）、total_rented_gpu 泄漏；
            //   (b) 破坏 do_machine_exit 依赖的不变量"DeepLink 机器处于 *ReportOffline ⟺ 快照已回退"，导致 exit 少回退。
            //   改硬件本就应在无租约时进行（原生 Rented 机器亦无法走到这里：is_online()=false）。链上强制拒绝。
            ensure!(!Self::deeplink_rented(&machine_id), Error::<T>::MachineStatusNotAllowed);
            machine_info.machine_status =
                MachineStatus::StakerReportOffline(now, Box::new(MachineStatus::Online));

            // 计算重新审核需要质押的支付给审核委员会的手续费
            let verify_fee =
                Self::cal_mut_hardware_stake().ok_or(Error::<T>::GetReonlineStakeFailed)?;
            // 计算下线的惩罚金额
            let offline_slash = Perbill::from_rational(4u32, 100u32) * machine_info.stake_amount;

            let total_stake = verify_fee.saturating_add(offline_slash);
            Self::change_stake(&machine_info.machine_stash, total_stake, true)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;

            UserMutHardwareStake::<T>::insert(
                &machine_info.machine_stash,
                &machine_id,
                UserMutHardwareStakeInfo {
                    verify_fee,
                    offline_slash,
                    offline_time: now,
                    need_fulfilling: false,
                },
            );

            Self::update_region_on_online_changed(&machine_info, false);
            // Will not fail, because machine_id check already
            Self::update_snap_on_online_changed(machine_id.clone(), false)
                .map_err(|_| Error::<T>::Unknown)?;

            LiveMachines::<T>::mutate(|live_machines| {
                live_machines.on_offline_change_hardware(machine_id.clone());
            });
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::MachineOfflineToMutHardware(
                machine_id,
                verify_fee,
                offline_slash,
            ));
            Ok(().into())
        }

        /// Controller account submit online request machine
        #[pallet::call_index(7)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn bond_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            msg: Vec<u8>,
            sig: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashBond)?;
            let now = <frame_system::Pallet<T>>::block_number();

            ensure!(!MachinesInfo::<T>::contains_key(&machine_id), Error::<T>::MachineIdExist);
            // 依赖stash_machine中的记录发放奖励。因此Machine退出后，仍保留
            let stash_machine = Self::stash_machines(&stash);
            ensure!(
                !stash_machine.total_machine.binary_search(&machine_id).is_ok(),
                Error::<T>::MachineIdExist
            );

            // 检查签名是否正确
            Self::check_bonding_msg(stash.clone(), machine_id.clone(), msg, sig)?;

            // 用户绑定机器需要质押一张显卡的DBC
            let stake_amount = Self::stake_per_gpu_v2().ok_or(Error::<T>::CalcStakeAmountFailed)?;
            // 扣除10个Dbc作为交易手续费; 并质押
            Self::pay_fixed_tx_fee(controller.clone())?;
            Self::change_stake(&stash, stake_amount, true)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;

            StashMachines::<T>::mutate(&stash, |stash_machines| {
                stash_machines.new_bonding(machine_id.clone());
            });
            LiveMachines::<T>::mutate(|live_machines| {
                live_machines.on_bonding(machine_id.clone());
            });
            ControllerMachines::<T>::mutate(&controller, |controller_machines| {
                ItemList::add_item(controller_machines, machine_id.clone());
            });
            let machine_info =
                MachineInfo::new_bonding(controller.clone(), stash, now, stake_amount);
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::BondMachine(controller, machine_id, stake_amount));
            Ok(().into())
        }

        /// Controller generate new server room id, record to stash account
        #[pallet::call_index(8)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn gen_server_room(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashBond)?;

            Self::pay_fixed_tx_fee(controller.clone())?;

            StashServerRooms::<T>::mutate(&stash, |stash_server_rooms| {
                let new_server_room = <generic_func::Pallet<T>>::random_server_room();
                ItemList::add_item(stash_server_rooms, new_server_room);
                Self::deposit_event(Event::ServerRoomGenerated(controller, new_server_room));
            });

            Ok(().into())
        }

        // NOTE: 添加机房信息。在机器上线之前的任何阶段及机器主动下线时，可以调用该方法更改机房信息
        /// Controller add machine pos & net info
        #[pallet::call_index(9)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn add_machine_info(
            origin: OriginFor<T>,
            machine_id: MachineId,
            server_room_info: StakerCustomizeInfo,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            // 查询机器Id是否在该账户的控制下
            let machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
            let stash = machine_info.machine_stash.clone();
            machine_info
                .can_add_server_room(&controller)
                .map_err::<Error<T>, _>(Into::into)?;

            let stash_server_rooms = Self::stash_server_rooms(&machine_info.machine_stash);
            ensure!(!server_room_info.telecom_operators.is_empty(), Error::<T>::TelecomIsNull);
            ensure!(
                stash_server_rooms.binary_search(&server_room_info.server_room).is_ok(),
                Error::<T>::ServerRoomNotFound
            );

            let is_reonline = UserMutHardwareStake::<T>::contains_key(&stash, &machine_id);
            if is_reonline {
                let mut reonline_stake = Self::user_mut_hardware_stake(&stash, &machine_id);
                if reonline_stake.verify_fee.is_zero() {
                    let verify_fee =
                        Self::cal_mut_hardware_stake().ok_or(Error::<T>::GetReonlineStakeFailed)?;
                    Self::change_stake(&stash, verify_fee, true)
                        .map_err(|_| Error::<T>::BalanceNotEnough)?;
                    reonline_stake.verify_fee = verify_fee;
                    UserMutHardwareStake::<T>::insert(&stash, &machine_id, reonline_stake);
                }
            }

            // 当是第一次上线添加机房信息时
            LiveMachines::<T>::mutate(|live_machines| {
                live_machines.on_add_server_room(machine_id.clone())
            });
            MachinesInfo::<T>::try_mutate(&machine_id, |machine_info| {
                let machine_info = machine_info.as_mut().ok_or(Error::<T>::Unknown)?;
                machine_info.add_server_room_info(server_room_info);
                Ok::<(), sp_runtime::DispatchError>(())
            })?;

            Self::deposit_event(Event::MachineInfoAdded(machine_id));
            Ok(().into())
        }

        // 机器第一次上线后处于补交质押状态时
        // 或者机器更改配置信息后，处于质押不足状态时, 需要补交质押才能上线
        #[pallet::call_index(10)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn fulfill_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Pallet<T>>::block_number();
            let current_era = Self::current_era();

            let mut machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
            let mut live_machine = Self::live_machines();

            ensure!(machine_info.is_controller(controller), Error::<T>::NotMachineController);
            ensure!(
                live_machine.fulfilling_machine.binary_search(&machine_id).is_ok(),
                Error::<T>::MachineStatusNotAllowed
            );

            // NOTE: 机器补交质押时，所需的质押 = max(当前机器需要的质押，第一次绑定上线时的质押量)
            // 每卡质押按照第一次上线时计算
            let stake_need = machine_info
                .init_stake_per_gpu
                .checked_mul(&machine_info.gpu_num().saturated_into::<BalanceOf<T>>())
                .ok_or(Error::<T>::CalcStakeAmountFailed)?;

            // 当出现需要补交质押时，补充质押并记录到机器信息中
            if machine_info.stake_amount < stake_need {
                let extra_stake = stake_need.saturating_sub(machine_info.stake_amount);
                Self::change_stake(&machine_info.machine_stash, extra_stake, true)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;
                machine_info.stake_amount = stake_need;
            }
            machine_info.machine_status = MachineStatus::Online;

            let is_reonline =
                UserMutHardwareStake::<T>::contains_key(&machine_info.machine_stash, &machine_id);
            if is_reonline {
                UserMutHardwareStake::<T>::remove(&machine_info.machine_stash, &machine_id);
            }
            // 当机器因为补交质押而上线时，不应该记录上线时间为Now
            machine_info.online_height = now;
            machine_info.reward_deadline = current_era + REWARD_DURATION;

            MachineRecentReward::<T>::insert(
                &machine_id,
                MachineRecentRewardInfo {
                    machine_stash: machine_info.machine_stash.clone(),
                    reward_committee_deadline: machine_info.reward_deadline,
                    reward_committee: machine_info.reward_committee.clone(),
                    recent_machine_reward: VecDeque::new(),
                    recent_reward_sum: 0u32.into(),
                },
            );

            machine_info.last_online_height = now;
            machine_info.last_machine_restake = now;

            Self::update_region_on_online_changed(&machine_info, true);
            Self::update_snap_on_online_changed(machine_id.clone(), true)
                .map_err(|_| Error::<T>::Unknown)?;

            ItemList::rm_item(&mut live_machine.fulfilling_machine, &machine_id);
            ItemList::add_item(&mut live_machine.online_machine, machine_id.clone());

            LiveMachines::<T>::put(live_machine);

            MachinesInfo::<T>::insert(&machine_id, machine_info);
            Ok(().into())
        }

        /// 控制账户进行领取收益到stash账户
        #[pallet::call_index(11)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashAccount)?;

            ensure!(StashMachines::<T>::contains_key(&stash), Error::<T>::NotMachineController);

            StashMachines::<T>::mutate(&stash, |stash_machine| -> DispatchResultWithPostInfo {
                let can_claim =
                    stash_machine.claim_reward().map_err(|_| Error::<T>::ClaimRewardFailed)?;

                let im_balance = <T as Config>::Currency::deposit_into_existing(&stash, can_claim)
                    .map_err(|_| Error::<T>::ClaimRewardFailed)?;
                drop(im_balance);

                Self::fulfill_machine_stake(stash.clone(), can_claim)
                    .map_err(|_| Error::<T>::ClaimThenFulfillFailed)?;
                Self::deposit_event(Event::ClaimReward(stash.clone(), can_claim));
                Ok(().into())
            })
        }

        /// 控制账户报告机器下线:Online/Rented时允许
        #[pallet::call_index(12)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn controller_report_offline(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Pallet<T>>::block_number();
            let machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;

            ensure!(machine_info.is_controller(controller), Error::<T>::NotMachineController);

            // 某些状态允许下线
            ensure!(
                matches!(
                    machine_info.machine_status,
                    MachineStatus::Online | MachineStatus::Rented
                ),
                Error::<T>::MachineStatusNotAllowed
            );

            // ③ 离线终止：记住离线前是否在租。
            let was_rented = matches!(machine_info.machine_status, MachineStatus::Rented);

            Self::machine_offline(
                machine_id.clone(),
                MachineStatus::StakerReportOffline(now, Box::new(machine_info.machine_status)),
            )
            .map_err(|_| Error::<T>::Unknown)?;

            // ③ 控制账户自报在租机器离线 → 同样终止其在租订单（罚≤24h、不碰 stake），与检测器路径一致。
            if was_rented {
                T::RentTerminate::settle_terminate_rents_on_offline(&machine_id);
            }

            Self::deposit_event(Event::ControllerReportOffline(machine_id));
            Ok(().into())
        }

        /// [Thread B ① · DLC 化] root 设置授权离线检测器集合（复用 DeepLink DDN 链上钱包）。
        #[pallet::call_index(29)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10_000, 0))]
        pub fn set_offline_detectors(
            origin: OriginFor<T>,
            detectors: Vec<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            OfflineDetectors::<T>::put(detectors);
            Self::deposit_event(Event::OfflineDetectorsUpdated);
            Ok(().into())
        }

        /// [Thread B ① · DLC 化] 授权检测器(DDN)上报机器离线——不要求机器被租（对齐 DLC 健康检测）。
        /// 对任意 Online/Rented 机器可报，走与 controller_report_offline 相同的 machine_offline 转换。
        /// 闲置机离线由 ② 零罚；被租机离线的租金惩罚由 ③ 托管处理；DeepLink 租用机离线会回退 +30%（桥已处理），
        /// 从而堵住"暗机白拿 +30%"（无需等 controller 自首）。链下 5min 防抖由 DDN 负责。
        #[pallet::call_index(30)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10_000, 0))]
        pub fn report_machine_offline_by_detector(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let detector = ensure_signed(origin)?;
            let now = <frame_system::Pallet<T>>::block_number();

            // fail-closed：仅授权检测器可调；空集则无人可调。
            ensure!(
                Self::offline_detectors().contains(&detector),
                Error::<T>::NotOfflineDetector
            );

            let machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
            // 只对在线/被租机器报离线（幂等：已离线机器无需再报）
            ensure!(
                matches!(
                    machine_info.machine_status,
                    MachineStatus::Online | MachineStatus::Rented
                ),
                Error::<T>::MachineStatusNotAllowed
            );

            // ③ 离线终止：先记住离线前是否在租（machine_offline 会把 status 改成 StakerReportOffline）。
            let was_rented = matches!(machine_info.machine_status, MachineStatus::Rented);

            // 复用 machine_offline：把当前状态包进 StakerReportOffline（与自报同处理，slash 由 ②/③ 决定）。
            Self::machine_offline(
                machine_id.clone(),
                MachineStatus::StakerReportOffline(now, Box::new(machine_info.machine_status)),
            )
            .map_err(|_| Error::<T>::Unknown)?;

            // ③ 在租机器被报离线 → 结算并终止其所有在租订单（罚≤24h 租金给租客、不碰 stake）。
            //   须在 machine_offline 之后调用（此时机器已处离线态，rent-machine 走离线分支只记 RentedFinished、
            //   不二次回退快照）。best-effort：内部不冒泡错误。
            if was_rented {
                T::RentTerminate::settle_terminate_rents_on_offline(&machine_id);
            }

            Self::deposit_event(Event::DetectorReportOffline(machine_id, detector));
            Ok(().into())
        }

        // NOTE: 如果机器主动下线/因举报下线之后，几个租用订单陆续到期，则机器主动上线
        // 要根据几个订单的状态来判断机器是否是在线/租用状态
        // 需要在rentMachine中提供一个查询接口
        /// 控制账户报告机器上线
        #[pallet::call_index(13)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn controller_report_online(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Pallet<T>>::block_number();

            let mut machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
            ensure!(machine_info.is_controller(controller), Error::<T>::NotMachineController);

            let mut live_machine = Self::live_machines();

            let status_before_offline: MachineStatus<T::BlockNumber, T::AccountId>;
            let offline_time = match machine_info.machine_status.clone() {
                MachineStatus::StakerReportOffline(offline_time, _) => offline_time,
                MachineStatus::ReporterReportOffline(slash_reason, ..) => match slash_reason {
                    OPSlashReason::RentedInaccessible(report_time) |
                    OPSlashReason::RentedHardwareMalfunction(report_time) |
                    OPSlashReason::RentedHardwareCounterfeit(report_time) |
                    OPSlashReason::OnlineRentFailed(report_time) => report_time,
                    _ => return Err(Error::<T>::MachineStatusNotAllowed.into()),
                },
                _ => return Err(Error::<T>::MachineStatusNotAllowed.into()),
            };
            let offline_duration = now.saturating_sub(offline_time);

            // MachineStatus改为之前的状态
            let mut slash_info = match machine_info.machine_status.clone() {
                MachineStatus::StakerReportOffline(offline_time, status) => {
                    status_before_offline = *status;
                    match status_before_offline {
                        MachineStatus::Online => Self::new_slash_when_offline(
                            machine_id.clone(),
                            OPSlashReason::OnlineReportOffline(offline_time),
                            None,
                            vec![],
                            None,
                            offline_duration,
                        ),
                        MachineStatus::Rented => Self::new_slash_when_offline(
                            machine_id.clone(),
                            OPSlashReason::RentedReportOffline(offline_time),
                            None,
                            machine_info.renters.clone(),
                            None,
                            offline_duration,
                        ),
                        _ => return Ok(().into()),
                    }
                },
                MachineStatus::ReporterReportOffline(slash_reason, status, reporter, committee) => {
                    status_before_offline = *status;
                    Self::new_slash_when_offline(
                        machine_id.clone(),
                        slash_reason,
                        Some(reporter),
                        Self::offline_machine_to_renters(&machine_id),
                        Some(committee),
                        offline_duration,
                    )
                },
                _ => return Err(Error::<T>::MachineStatusNotAllowed.into()),
            }
            .map_err(|_| Error::<T>::Unknown)?;

            if let MachineStatus::ReporterReportOffline(slash_reason, ..) =
                machine_info.machine_status.clone()
            {
                if crate::utils::reach_max_slash(
                    &slash_reason,
                    offline_duration.saturated_into::<u64>(),
                ) {
                    let ever_slashed = Self::max_slash_execed(&machine_id);
                    if ever_slashed > offline_time && ever_slashed < now {
                        slash_info.slash_amount = Zero::zero();
                    }
                }
            }

            // NOTE: 如果机器上线超过一年，空闲超过10天，下线后上线不添加惩罚
            if now >= machine_info.online_height &&
                now.saturating_sub(machine_info.online_height) > (365 * ONE_DAY).into() &&
                offline_time >= machine_info.last_online_height &&
                offline_time.saturating_sub(machine_info.last_online_height) >=
                    (10 * ONE_DAY).into() &&
                matches!(&machine_info.machine_status, &MachineStatus::StakerReportOffline(..))
            {
                slash_info.slash_amount = Zero::zero();
            }

            // machine status before offline
            machine_info.last_online_height = now;
            machine_info.machine_status = if RentedFinished::<T>::contains_key(&machine_id) {
                MachineStatus::Online
            } else {
                status_before_offline
            };

            // 添加下线惩罚
            if slash_info.slash_amount != Zero::zero() {
                // 任何情况重新上链都需要补交质押
                Self::change_stake(&machine_info.machine_stash, slash_info.slash_amount, true)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;

                // NOTE: Only after pay slash amount succeed, then make machine online.
                let slash_id = Self::get_new_slash_id();
                PendingExecSlash::<T>::mutate(slash_info.slash_exec_time, |pending_exec_slash| {
                    ItemList::add_item(pending_exec_slash, slash_id);
                });
                PendingSlash::<T>::insert(slash_id, slash_info);

                Machine2PendingSlashIds::<T>::mutate(&machine_id, |slash_ids| {
                    ItemList::add_item(slash_ids, slash_id);
                });

                Self::deposit_event(Event::AddSlash(machine_id.clone(), slash_id));
            }

            ItemList::rm_item(&mut live_machine.offline_machine, &machine_id);

            Self::update_snap_on_online_changed(machine_id.clone(), true)
                .map_err(|_| Error::<T>::Unknown)?;
            Self::update_region_on_online_changed(&machine_info, true);
            if machine_info.machine_status == MachineStatus::Rented {
                ItemList::add_item(&mut live_machine.rented_machine, machine_id.clone());
                Self::update_snap_on_rent_changed(machine_id.clone(), true)
                    .map_err(|_| Error::<T>::Unknown)?;
                Self::update_region_on_rent_changed(&machine_info, true);
            } else if Self::deeplink_rented(&machine_id) {
                // [审计修 #5a] DeepLink 租用：重新上线时恢复 rent 快照(total_rented_gpu/+30%)，与 machine_offline
                //   的回退对称。⚠️ 但**不**加入 live_machine.rented_machine——DeepLink 不管理该列表(EVM endRent
                //   只 toggle 快照，进了 rented_machine 没有路径移除会卡死)，仍归 online_machine。
                ItemList::add_item(&mut live_machine.online_machine, machine_id.clone());
                Self::update_snap_on_rent_changed(machine_id.clone(), true)
                    .map_err(|_| Error::<T>::Unknown)?;
                Self::update_region_on_rent_changed(&machine_info, true);
            } else {
                ItemList::add_item(&mut live_machine.online_machine, machine_id.clone());
            }

            // Try to remove frm rentedFinished
            RentedFinished::<T>::remove(&machine_id);
            LiveMachines::<T>::put(live_machine);
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::ControllerReportOnline(machine_id));
            Ok(().into())
        }

        /// 超过365天的机器可以在距离上次租用10天，且没被租用时退出
        #[pallet::call_index(14)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn machine_exit(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
            let now = <frame_system::Pallet<T>>::block_number();
            let current_era = Self::current_era();

            ensure!(machine_info.is_controller(controller), Error::<T>::NotMachineController);
            ensure!(machine_info.is_online(), Error::<T>::MachineStatusNotAllowed);
            // 确保机器已经上线一年：即reward_deadline - 365 <= current_era
            ensure!(machine_info.reward_deadline <= current_era + 365, Error::<T>::TimeNotAllowed);
            // 确保机器距离上次租用超过10天
            ensure!(
                now.saturating_sub(machine_info.last_online_height) >= (10 * ONE_DAY).into(),
                Error::<T>::TimeNotAllowed
            );

            Self::do_machine_exit(machine_id, machine_info)
        }

        /// 满足365天可以申请重新质押，退回质押币
        /// 在系统中上线满365天之后，可以按当时机器需要的质押数量，重新入网。多余的币解绑
        /// 在重新上线之后，下次再执行本操作，需要等待365天
        #[pallet::call_index(15)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn restake_online_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Pallet<T>>::block_number();
            let mut machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
            let pre_stake = machine_info.stake_amount;

            ensure!(machine_info.is_controller(controller), Error::<T>::NotMachineController);
            ensure!(
                now.saturating_sub(machine_info.last_machine_restake) >= REBOND_FREQUENCY.into(),
                Error::<T>::TooFastToReStake
            );
            let stake_per_gpu = Self::stake_per_gpu().ok_or(Error::<T>::CalcStakeAmountFailed)?;
            let stake_need = stake_per_gpu
                .checked_mul(&machine_info.gpu_num().saturated_into::<BalanceOf<T>>())
                .ok_or(Error::<T>::CalcStakeAmountFailed)?;

            // 年度核算：多退少不补
            if machine_info.stake_amount > stake_need {
                // 多退：退还多余质押
                let extra_stake = machine_info
                    .stake_amount
                    .checked_sub(&stake_need)
                    .ok_or(Error::<T>::ReduceStakeFailed)?;

                Self::change_stake(&machine_info.machine_stash, extra_stake, false)
                    .map_err(|_| Error::<T>::ReduceStakeFailed)?;
                machine_info.stake_amount = stake_need;
            }
            // 少不补：如果 stake_amount <= stake_need，不要求卡主补差额
            // 质押不足的部分继续由在线奖励自动填充（fulfill_machine_stake）

            machine_info.last_machine_restake = now;
            machine_info.init_stake_per_gpu = stake_per_gpu;

            MachinesInfo::<T>::insert(&machine_id, machine_info.clone());

            Self::deposit_event(Event::MachineRestaked(machine_id, pre_stake, stake_need));
            Ok(().into())
        }

        #[pallet::call_index(16)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn apply_slash_review(
            origin: OriginFor<T>,
            slash_id: SlashId,
            reason: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Pallet<T>>::block_number();

            let slash_info = Self::pending_slash(slash_id).ok_or(Error::<T>::Unknown)?;
            let machine_info =
                Self::machines_info(&slash_info.machine_id).ok_or(Error::<T>::Unknown)?;
            let online_stake_params =
                Self::online_stake_params().ok_or(Error::<T>::GetReonlineStakeFailed)?;

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);
            ensure!(slash_info.slash_exec_time > now, Error::<T>::ExpiredSlash);

            // 补交质押
            Self::change_stake(
                &machine_info.machine_stash,
                online_stake_params.slash_review_stake,
                true,
            )
            .map_err(|_| Error::<T>::BalanceNotEnough)?;

            PendingSlashReview::<T>::insert(
                slash_id,
                OPPendingSlashReviewInfo {
                    applicant: controller,
                    staked_amount: online_stake_params.slash_review_stake,
                    apply_time: now,
                    expire_time: slash_info.slash_exec_time,
                    reason,
                },
            );

            PendingSlashReviewChecking::<T>::mutate(
                slash_info.slash_exec_time,
                |pending_review_checking| {
                    ItemList::add_item(pending_review_checking, slash_id);
                },
            );

            Self::deposit_event(Event::ApplySlashReview(slash_id));
            Ok(().into())
        }

        #[pallet::call_index(17)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn cancel_slash(origin: OriginFor<T>, slash_id: u64) -> DispatchResultWithPostInfo {
            T::CancelSlashOrigin::ensure_origin(origin)?;
            Self::do_cancel_slash(slash_id)
        }

        #[pallet::call_index(18)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn exec_slash(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let _ = ensure_signed(origin)?;
            let now = <frame_system::Pallet<T>>::block_number();
            let machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;

            let offline_time = match machine_info.machine_status.clone() {
                MachineStatus::StakerReportOffline(_offline_time, _) => {
                    return Err(Error::<T>::MachineStatusNotAllowed.into())
                },
                MachineStatus::ReporterReportOffline(slash_reason, ..) => match slash_reason {
                    OPSlashReason::RentedInaccessible(report_time) |
                    OPSlashReason::RentedHardwareMalfunction(report_time) |
                    OPSlashReason::RentedHardwareCounterfeit(report_time) |
                    OPSlashReason::OnlineRentFailed(report_time) => {
                        // 确保机器达到最大惩罚量时，才允许调用
                        let offline_duration = now.saturating_sub(report_time);
                        if !crate::utils::reach_max_slash(
                            &slash_reason,
                            offline_duration.saturated_into::<u64>(),
                        ) {
                            return Err(Error::<T>::MachineStatusNotAllowed.into())
                        }

                        let ever_slashed = Self::max_slash_execed(&machine_id);
                        if ever_slashed > report_time && ever_slashed < now {
                            return Err(Error::<T>::MachineStatusNotAllowed.into())
                        }
                        report_time
                    },
                    _ => return Err(Error::<T>::MachineStatusNotAllowed.into()),
                },
                _ => return Err(Error::<T>::MachineStatusNotAllowed.into()),
            };
            let offline_duration = now.saturating_sub(offline_time);

            // MachineStatus改为之前的状态
            let slash_info = match machine_info.machine_status.clone() {
                MachineStatus::ReporterReportOffline(
                    slash_reason,
                    _status,
                    reporter,
                    committee,
                ) => {
                    // let status_before_offline = *status;
                    Self::new_slash_when_offline(
                        machine_id.clone(),
                        slash_reason,
                        Some(reporter),
                        Self::offline_machine_to_renters(&machine_id),
                        Some(committee),
                        offline_duration,
                    )
                },
                _ => return Err(Error::<T>::MachineStatusNotAllowed.into()),
            }
            .map_err(|_| Error::<T>::Unknown)?;

            // 添加下线惩罚
            if slash_info.slash_amount != Zero::zero() {
                let slash_id = Self::get_new_slash_id();
                PendingExecSlash::<T>::mutate(slash_info.slash_exec_time, |pending_exec_slash| {
                    ItemList::add_item(pending_exec_slash, slash_id);
                });
                PendingSlash::<T>::insert(slash_id, slash_info);

                Machine2PendingSlashIds::<T>::mutate(&machine_id, |slash_ids| {
                    ItemList::add_item(slash_ids, slash_id);
                });
                Self::deposit_event(Event::AddSlash(machine_id.clone(), slash_id));
            }

            MaxSlashExeced::<T>::insert(machine_id, now);
            Ok(().into())
        }

        #[pallet::call_index(19)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn update_machine_info(
            origin: OriginFor<T>,
            machine_id: MachineId,
            server_room_info: StakerCustomizeInfo,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            // check if the machine id is under control of the account
            let machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
            machine_info
                .can_update_server_room_info(&controller)
                .map_err::<Error<T>, _>(Into::into)?;

            let stash_server_rooms = Self::stash_server_rooms(&machine_info.machine_stash);
            ensure!(!server_room_info.telecom_operators.is_empty(), Error::<T>::TelecomIsNull);
            ensure!(
                stash_server_rooms.binary_search(&server_room_info.server_room).is_ok(),
                Error::<T>::ServerRoomNotFound
            );

            MachinesInfo::<T>::try_mutate(&machine_id, |machine_info| {
                let machine_info = machine_info.as_mut().ok_or(Error::<T>::Unknown)?;
                machine_info.add_server_room_info(server_room_info);
                Ok::<(), DispatchError>(())
            })?;

            Self::deposit_event(Event::MachineInfoUpdated(machine_id));
            Ok(().into())
        }

        #[pallet::call_index(20)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn force_machine_exit(
            origin: OriginFor<T>,
            machine_ids: Vec<MachineId>,
        ) -> DispatchResultWithPostInfo {
            let signer = ensure_signed(origin)?;

            // check authorization
            let authorized_accounts = Self::authorized_force_exit_accounts();
            ensure!(authorized_accounts.contains(&signer), Error::<T>::NotAuthorized);

            for machine_id in machine_ids {
                let machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
                Self::do_machine_exit(machine_id, machine_info)?;
            }

            Ok(().into())
        }

        #[pallet::call_index(21)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn update_authorized_force_exit_accounts(
            origin: OriginFor<T>,
            accounts: Vec<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            AuthorizedForceExitAccounts::<T>::put(accounts);
            Ok(().into())
        }

        /// 卡主设置机器额外加价（在系统自动定价基础上叠加）
        /// 单位：USD×10^6 per day per GPU，与 get_machine_price 返回值单位一致
        /// 设置为 0 表示不额外加价
        /// 上限 10,000 USD per day per GPU，防止设置天价 DoS 租户和计算溢出
        /// Weight: 2 reads (machines_info, machine_extra_price) + 1 write + 1 event
        /// Estimated ~150_000 ref_time; TODO: 运行 runtime-benchmarks 校准实际值
        #[pallet::call_index(22)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(150_000, 0))]
        pub fn set_machine_extra_price(
            origin: OriginFor<T>,
            machine_id: MachineId,
            extra_price: u64,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
            // 只有卡主(stash)或控制者(controller)可以设置
            ensure!(
                machine_info.machine_stash == who || machine_info.controller == who,
                Error::<T>::NotMachineController
            );
            // 上限：$10,000 per day per GPU
            const MAX_EXTRA_PRICE: u64 = 10_000_000_000;
            ensure!(extra_price <= MAX_EXTRA_PRICE, Error::<T>::ExtraPriceTooHigh);
            MachineExtraPrice::<T>::insert(&machine_id, extra_price);
            Self::deposit_event(Event::MachineExtraPriceSet(machine_id, extra_price));
            Ok(().into())
        }

        /// 切换机器租赁模式（全天 / 按时段）
        /// 模式切换不改变已质押数量，但新机器绑定时按模式决定起步质押
        /// 限制：机器正在租用中时不允许切换模式（避免租赁保证被破坏）
        /// Weight: 1 read (machines_info) + 1 write + 1 event ~ 140_000 ref_time
        #[pallet::call_index(23)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(140_000, 0))]
        pub fn set_machine_rental_mode(
            origin: OriginFor<T>,
            machine_id: MachineId,
            mode: MachineRentalMode,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
            ensure!(
                machine_info.machine_stash == who || machine_info.controller == who,
                Error::<T>::NotMachineController
            );
            // 机器正在租用时不允许切换模式
            ensure!(
                machine_info.machine_status != MachineStatus::Rented,
                Error::<T>::MachineStatusNotAllowed
            );
            MachineRentalModeStorage::<T>::insert(&machine_id, mode);
            Self::deposit_event(Event::MachineRentalModeSet(machine_id, mode));
            Ok(().into())
        }

        /// 设置机器某一天的每周循环时段（weekday: 0=周日 .. 6=周六）
        /// 时间为 UTC 小时值；start_hour < end_hour；end_hour 最大 24
        /// 传入空 Vec 表示该天不出租
        /// ranges 上限 10 个；不允许时段重叠
        /// Weight: 1 read + ranges 校验 O(n²) n≤10 + 1 write [Vec×7 整数组] + 1 event ~ 200_000
        #[pallet::call_index(24)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(200_000, 0))]
        pub fn set_weekly_schedule(
            origin: OriginFor<T>,
            machine_id: MachineId,
            weekday: u8,
            ranges: Vec<TimeRange>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
            ensure!(
                machine_info.machine_stash == who || machine_info.controller == who,
                Error::<T>::NotMachineController
            );
            ensure!(weekday < 7, Error::<T>::InvalidScheduleArgs);
            for r in ranges.iter() {
                ensure!(r.is_valid(), Error::<T>::InvalidScheduleArgs);
            }
            // 校验 range 数量上限（防止存储膨胀）
            const MAX_RANGES_PER_DAY: usize = 10;
            ensure!(ranges.len() <= MAX_RANGES_PER_DAY, Error::<T>::InvalidScheduleArgs);
            // 校验 range 不重叠
            ensure!(Self::ranges_are_disjoint(&ranges), Error::<T>::InvalidScheduleArgs);
            WeeklySchedule::<T>::mutate(&machine_id, |schedule| {
                schedule[weekday as usize] = ranges.clone();
            });
            Self::deposit_event(Event::WeeklyScheduleSet(machine_id, weekday));
            Ok(().into())
        }

        /// 设置机器特定日期的时段（优先级高于每周循环）
        /// date_days: 自 UNIX epoch 起的天数
        /// 传空 Vec → 等同于 clear_specific_date：删除设置，回退到每周循环
        /// ranges 上限 10 个；日期必须是过去 30 天至未来 365 天内
        /// Weight: 2 reads (machines_info, timestamp) + 校验 + 1 write + 1 event ~ 180_000
        #[pallet::call_index(25)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(180_000, 0))]
        pub fn set_specific_date_schedule(
            origin: OriginFor<T>,
            machine_id: MachineId,
            date_days: u32,
            ranges: Vec<TimeRange>,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
            ensure!(
                machine_info.machine_stash == who || machine_info.controller == who,
                Error::<T>::NotMachineController
            );

            // 空 Vec 表示清除该日期设置（回退到每周循环）
            if ranges.is_empty() {
                SpecificDateSchedule::<T>::remove(&machine_id, date_days);
                Self::deposit_event(Event::SpecificDateCleared(machine_id, date_days));
                return Ok(().into())
            }

            // 校验每个 TimeRange
            for r in ranges.iter() {
                ensure!(r.is_valid(), Error::<T>::InvalidScheduleArgs);
            }
            // 校验 range 数量上限（防止存储膨胀）
            const MAX_RANGES_PER_DAY: usize = 10;
            ensure!(ranges.len() <= MAX_RANGES_PER_DAY, Error::<T>::InvalidScheduleArgs);
            // 校验 range 不重叠
            ensure!(Self::ranges_are_disjoint(&ranges), Error::<T>::InvalidScheduleArgs);
            // 校验日期范围：过去 30 天至未来 365 天（防止设置天数膨胀）
            let today = (Self::current_time_ms() / 86_400_000) as u32;
            ensure!(
                date_days.saturating_add(30) >= today
                    && date_days <= today.saturating_add(365),
                Error::<T>::InvalidScheduleArgs
            );

            SpecificDateSchedule::<T>::insert(&machine_id, date_days, ranges);
            Self::deposit_event(Event::SpecificDateScheduleSet(machine_id, date_days));
            Ok(().into())
        }

        /// 清除机器特定日期的时段（回退到每周循环）
        /// Weight: 1 read + 1 remove + 1 event ~ 140_000
        #[pallet::call_index(26)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(140_000, 0))]
        pub fn clear_specific_date(
            origin: OriginFor<T>,
            machine_id: MachineId,
            date_days: u32,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
            ensure!(
                machine_info.machine_stash == who || machine_info.controller == who,
                Error::<T>::NotMachineController
            );
            SpecificDateSchedule::<T>::remove(&machine_id, date_days);
            Self::deposit_event(Event::SpecificDateCleared(machine_id, date_days));
            Ok(().into())
        }

        /// 矿工设置独立收租钱包（spec 410）
        /// 传 None 恢复默认（租金走 stash 账户）。仅 stash 本人可调用。
        /// Weight: 1 write + 1 event ~ 140_000
        #[pallet::call_index(27)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(140_000, 0))]
        pub fn set_rent_receiver(
            origin: OriginFor<T>,
            receiver: Option<T::AccountId>,
        ) -> DispatchResultWithPostInfo {
            let stash = ensure_signed(origin)?;
            // spec 410: 防止把租金发到全零地址（实际上等于烧钱，UX 陷阱）
            if let Some(ref r) = receiver {
                let zero = T::AccountId::decode(&mut &[0u8; 32][..])
                    .map_err(|_| Error::<T>::InvalidRentReceiver)?;
                ensure!(r != &zero, Error::<T>::InvalidRentReceiver);
            }
            match receiver.as_ref() {
                Some(r) => StashRentReceiver::<T>::insert(&stash, r),
                None => StashRentReceiver::<T>::remove(&stash),
            }
            Self::deposit_event(Event::RentReceiverChanged(stash, receiver));
            Ok(().into())
        }

        /// [审计修 H-1] 运维强制设置 DeepLinkRented 应急阀（root only）。
        /// 解决：EVM endRent 的桥 `deeplink_set_rented(false)` 在机器注销/machine_info 缺失时失败被 try/catch 吞，
        /// `DeepLinkRented` 卡 true → `change_machine_status_on_rent_start` 守卫永久拒原生租用 + 矿工 +30% 泄漏，
        /// 而 `deeplink_set_rented` 仅由 EVM 桥调、无任何链上清除手段。本阀强制落标记并尽力对账快照
        /// （机器已注销时 `update_snap_on_rent_changed` 会 Err，此时本就无 era 快照可回滚→仅清标记，reconciled=false）。
        /// ⚠️ 共识相关：动挖矿被租会计，须经 DBC 团队评审 + 测试网验证后再上主网。
        #[pallet::call_index(28)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(200_000, 0))]
        pub fn force_set_deeplink_rented(
            origin: OriginFor<T>,
            machine_id: MachineId,
            is_rented: bool,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            // [评审修] 即使 root 强制路径，强制 is_rented=true 也必须遵守互斥守卫②——否则给一台已被原生
            //   租用(MachineRentedGPU>0)的机器强打 DeepLink 租用 → is_rented/total_rented_gpu/+30% double-count。
            //   force-false(清除卡死)永远放行，不受此限。
            if is_rented {
                ensure!(MachineRentedGPU::<T>::get(&machine_id) == 0, Error::<T>::MachineNativelyRented);
                // [审计修 #5c round2] force-true 必须复用 precompile 路径的全部前置，否则 root 手滑可绕过：
                //   (1) terminating-rental 互斥——对已被 terminating 租用的机器强打 DeepLink → 跨系统 +30% double-count；
                //   (2) 在线前置——对非 Online 机器(CommitteeVerifying/WaitingFulfill/离线等)强打 → apply 会给 era+1 快照
                //       注入幽灵被租条目(+30% 进 total 但机器从未走 online_changed(true))→ 分母虚高稀释全网、随 era 传播。
                //   force-false(清卡死)不受此限，见下。
                ensure!(
                    !T::TerminatingRentalStatus::is_machine_rented(&machine_id),
                    Error::<T>::MachineTerminatingRented
                );
                //   在线前置须对齐 apply_deeplink_rented 的 skip_snap 语义，而非死卡 Online：
                //   · Online → apply 立即施加快照（安全）；
                //   · StakerReportOffline/ReporterReportOffline（曾在线、现离线）→ apply 感知离线跳过快照、只落标记，
                //     等 controller_report_online 恰好施加一次（这是 round3 阀门的合法用法：离线期间强制补标记）；
                //   · 其余"从未在线"态(AddingCustomizeInfo/CommitteeVerifying/WaitingFulfill/…)→ skip_snap=false →
                //     update_snap 会给从未 online_changed(true) 的机器注入幽灵被租快照(+30% 进 total、机器却无点数)→ 拒绝。
                ensure!(
                    matches!(
                        Self::machines_info(&machine_id).map(|mi| mi.machine_status),
                        Some(MachineStatus::Online)
                            | Some(MachineStatus::StakerReportOffline(..))
                            | Some(MachineStatus::ReporterReportOffline(..))
                    ),
                    Error::<T>::MachineNotOnlineForDeepLink
                );
            }
            // [评审修 round2] 复用 apply_deeplink_rented 的离线/注销感知：force-false 时若机器离线(快照已被
            //   machine_offline 回退)或已注销→只清标记不再回退快照，防 double-减 total_rented_gpu/grade。
            //   reconciled = 是否成功落标记（false 路径恒 Ok；true 路径机器注销时 update_snap Err → false）。
            let reconciled = Self::apply_deeplink_rented(&machine_id, is_rented).is_ok();
            Self::deposit_event(Event::ForceSetDeepLinkRented(machine_id, is_rented, reconciled));
            Ok(().into())
        }
    }

    #[pallet::event]
    // #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        BondMachine(T::AccountId, MachineId, BalanceOf<T>),
        Slash(T::AccountId, BalanceOf<T>, OPSlashReason<T::BlockNumber>),
        ControllerStashBonded(T::AccountId, T::AccountId),
        // 弃用
        MachineControllerChanged(MachineId, T::AccountId, T::AccountId),
        // (MachineId, reward to verify committee, offline slash)
        MachineOfflineToMutHardware(MachineId, BalanceOf<T>, BalanceOf<T>),
        StakeAdded(T::AccountId, BalanceOf<T>),
        StakeReduced(T::AccountId, BalanceOf<T>),
        ServerRoomGenerated(T::AccountId, H256),
        MachineInfoAdded(MachineId),
        MachineInfoUpdated(MachineId),
        ClaimReward(T::AccountId, BalanceOf<T>),
        ControllerReportOffline(MachineId),
        ControllerReportOnline(MachineId),
        SlashCanceled(u64, T::AccountId, BalanceOf<T>),
        // machine_id, old_stake, new_stake
        MachineRestaked(MachineId, BalanceOf<T>, BalanceOf<T>),
        MachineExit(MachineId),
        // Slash_who, reward_who, reward_amount
        SlashAndReward(T::AccountId, T::AccountId, BalanceOf<T>, OPSlashReason<T::BlockNumber>),
        ApplySlashReview(SlashId),
        SlashExecuted(T::AccountId, MachineId, BalanceOf<T>),
        NewSlash(SlashId),
        SetTmpVal(u64),
        // stash, pre_controller, post_controller
        StashResetController(T::AccountId, T::AccountId, T::AccountId),
        // machine_id, pre_stake, delta_stake
        MachineAddStake(MachineId, BalanceOf<T>, BalanceOf<T>),
        AddSlash(MachineId, SlashId),
        // NEW in spec 408 - must be at the END to preserve existing event positions
        // machine_id, extra_price (USD×10^6 per day per GPU)
        MachineExtraPriceSet(MachineId, u64),
        // NEW in spec 409 - time-slot rental (all added at END for ABI compat)
        MachineRentalModeSet(MachineId, MachineRentalMode),
        // machine_id, weekday (0-6)
        WeeklyScheduleSet(MachineId, u8),
        // machine_id, date_days (since epoch)
        SpecificDateScheduleSet(MachineId, u32),
        SpecificDateCleared(MachineId, u32),
        // spec 410: 矿工设置独立收租钱包；Some(addr)=切换，None=恢复默认（stash 收）
        RentReceiverChanged(T::AccountId, Option<T::AccountId>),
        // spec 413: 罚没实际扣减额 < 请求额 (储备不足, best-effort 罚没) 时发出。
        // (slash_who, requested, actually_slashed)
        SlashShortfall(T::AccountId, BalanceOf<T>, BalanceOf<T>),
        // spec 413: confirm_machine 上线快照更新失败 (低概率) 时发出。
        SnapshotUpdateFailed(MachineId),
        // [审计修 H-1] 运维强制设置 DeepLinkRented 应急阀：(machine_id, is_rented, snapshot_reconciled)。
        //   ⚠️ 必须放枚举末尾以保留既有事件的 SCALE 判别式位置（链下索引器/dbcscan 按位置解码）。
        //   放在 spec 413 的 SlashShortfall/SnapshotUpdateFailed 之后，保留 413 事件在主网的位置。
        ForceSetDeepLinkRented(MachineId, bool, bool),
        /// [Thread B ①] 授权离线检测器集合被更新（SCALE 追加于末尾）
        OfflineDetectorsUpdated,
        /// [Thread B ①] 检测器上报机器离线 (machine_id, detector)（SCALE 追加于末尾）
        DetectorReportOffline(MachineId, T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        BadSignature,
        MachineIdExist,
        BalanceNotEnough,
        NotMachineController,
        PayTxFeeFailed,
        ClaimRewardFailed,
        ConvertMachineIdToWalletFailed,
        NoStashBond,
        AlreadyController,
        NoStashAccount,
        BadMsgLen,
        NotAllowedChangeMachineInfo,
        MachineStashNotEqualControllerStash,
        CalcStakeAmountFailed,
        SigMachineIdNotEqualBondedMachineId,
        TelecomIsNull,
        MachineStatusNotAllowed,
        ServerRoomNotFound,
        NotMachineStash,
        TooFastToReStake,
        NoStakeToReduce,
        ReduceStakeFailed,
        GetReonlineStakeFailed,
        SlashIdNotExist,
        TimeNotAllowed,
        ExpiredSlash,
        Unknown,
        ClaimThenFulfillFailed,
        /// 账户未被授权执行此操作
        NotAuthorized,
        /// 额外加价超过上限
        ExtraPriceTooHigh,
        /// 时段参数不合法
        InvalidScheduleArgs,
        /// 租用时长不足最小要求（2小时）
        RentalTooShort,
        /// 请求时段不在机器允许出租的时段内
        OutOfRentalSchedule,
        /// spec 410: receiver 地址非法（如全零）
        InvalidRentReceiver,
        /// [评审修 H-1] 机器已被原生 rent-machine 租用，禁止 force_set_deeplink_rented 强打 DeepLink 租用（防 double-count）
        MachineNativelyRented,
        /// [审计修 #5c round2] 机器已被 terminating-rental 租用，禁止 force_set_deeplink_rented 强打 DeepLink 租用（跨系统 double-count）
        MachineTerminatingRented,
        /// [审计修 #5c round2] force_set_deeplink_rented(true) 要求机器 == Online（对齐 precompile 路径的在线前置，防给非在线机注入幽灵快照）
        MachineNotOnlineForDeepLink,
        /// [Thread B ①] 调用者不在授权离线检测器集合内
        NotOfflineDetector,
    }
}

impl<T: Config> Pallet<T> {
    // 计算重新审核需要质押的支付给审核委员会的手续费
    pub fn cal_mut_hardware_stake() -> Option<BalanceOf<T>> {
        let online_stake_params = Self::online_stake_params()?;
        T::DbcPrice::get_dbc_amount_by_value(online_stake_params.reonline_stake)
    }

    /// spec 410: 获取 stash 的实际收租账户（未配置则回退到 stash 本身）
    pub fn effective_rent_receiver(stash: &T::AccountId) -> T::AccountId {
        Self::stash_rent_receiver(stash).unwrap_or_else(|| stash.clone())
    }

    // ═══════════════════════════════════════════════════════════════
    // 分时段出租：校验请求时段是否被机器允许
    // ═══════════════════════════════════════════════════════════════

    /// 把毫秒时间戳拆成 (date_days, weekday, hour_of_day)
    /// weekday: 0=周日, 1=周一 ... 6=周六（UNIX epoch 1970-01-01 是周四 = 4）
    fn split_timestamp(ts_ms: u64) -> (u32, u8, u8) {
        let secs = ts_ms / 1000;
        let date_days = (secs / 86400) as u32;
        let weekday = ((date_days + 4) % 7) as u8;
        let hour_of_day = ((secs % 86400) / 3600) as u8;
        (date_days, weekday, hour_of_day)
    }

    /// 返回机器在指定日期（UNIX 天数）的可用时段列表
    /// 特定日期优先于每周循环
    fn available_ranges_on_date(
        machine_id: &MachineId,
        date_days: u32,
        weekday: u8,
    ) -> Vec<TimeRange> {
        if SpecificDateSchedule::<T>::contains_key(machine_id, date_days) {
            SpecificDateSchedule::<T>::get(machine_id, date_days)
        } else {
            let schedule = WeeklySchedule::<T>::get(machine_id);
            schedule.get(weekday as usize).cloned().unwrap_or_default()
        }
    }

    /// 校验请求的租用时间段 [start_ts_ms, end_ts_ms) 是否在机器的可用时段内
    /// 仅对 TimeSlot 模式生效。FullTime 模式总是返回 true。
    /// 要求：
    /// - 时长至少 2 小时
    /// - 必须完全落在同一天的某个时段内（不支持跨日）
    pub fn is_rental_schedule_allowed(
        machine_id: &MachineId,
        start_ts_ms: u64,
        end_ts_ms: u64,
    ) -> bool {
        // FullTime 模式不做时段限制
        if Self::machine_rental_mode(machine_id) == MachineRentalMode::FullTime {
            return true
        }

        if end_ts_ms <= start_ts_ms {
            return false
        }

        // 最小 2 小时（毫秒）
        const MIN_DURATION_MS: u64 = 2 * 60 * 60 * 1000;
        if end_ts_ms - start_ts_ms < MIN_DURATION_MS {
            return false
        }

        let (start_date, start_weekday, start_hour) = Self::split_timestamp(start_ts_ms);
        let end_secs = end_ts_ms / 1000;
        let end_date = (end_secs / 86400) as u32;
        let end_hour_in_day = ((end_secs % 86400) / 3600) as u8;
        // 如果租期跨越 UTC 日（例如 23:00 到次日 01:00），要求严格单日内
        // 但允许结束正好是 00:00（即 end_hour = 24 当日视角）
        let end_hour_normalized = if end_date == start_date {
            end_hour_in_day
        } else if end_date == start_date + 1 && end_secs % 86400 == 0 {
            24
        } else {
            return false
        };

        let ranges = Self::available_ranges_on_date(machine_id, start_date, start_weekday);
        for range in ranges.iter() {
            if range.covers(start_hour, end_hour_normalized) {
                return true
            }
        }
        false
    }

    /// 当前链上时间（毫秒）
    pub fn current_time_ms() -> u64 {
        <pallet_timestamp::Pallet<T>>::get().saturated_into::<u64>()
    }

    /// 校验时段列表不重叠（两两检查，O(n²)，n 最大 10 无性能问题）
    pub fn ranges_are_disjoint(ranges: &[TimeRange]) -> bool {
        for i in 0..ranges.len() {
            for j in (i + 1)..ranges.len() {
                let a = ranges[i];
                let b = ranges[j];
                // 有交集：a.start < b.end && b.start < a.end
                if a.start_hour < b.end_hour && b.start_hour < a.end_hour {
                    return false
                }
            }
        }
        true
    }

    // NOTE: StashMachine.total_machine cannot be removed. Because Machine will be rewarded in 150 eras.
    pub fn do_machine_exit(
        machine_id: MachineId,
        machine_info: MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
    ) -> DispatchResultWithPostInfo {
        // 下线机器，并退还奖励
        Self::change_stake(&machine_info.machine_stash, machine_info.stake_amount, false)
            .map_err(|_| Error::<T>::ReduceStakeFailed)?;

        // FIX: when force-exiting a Rented machine, total_rented_gpu must be
        // decremented BEFORE total_gpu_num. Without this, sys_info ends up with
        // total_rented_gpu > total_gpu_num (structurally impossible) over time
        // — observed on mainnet as totalRentedGpu=96 > totalGpuNum=93.
        // [审计修 #5a] 同 machine_offline：DeepLink 租用的机器 machine_status 不是 Rented，force-exit 时若不回退
        //   会留下 total_rented_gpu 多计（正是注释记录的 totalRentedGpu>totalGpuNum 主网症状的 DeepLink 版本）。
        // [审计修 #5c — round2 HIGH] 只有"快照当前仍处于被租状态"时才回退，否则会 double-rollback：
        //   · 原生 Rented：机器离线后 machine_status 从 Rented 变为 *ReportOffline，此分支自然跳过（已由 machine_offline 回退）。
        //   · DeepLink：machine_offline 在机器【在线】时就已回退 +30%/total_rented_gpu 但【保留 flag=true】(等 online 恢复)。
        //     若这里仅凭 flag 回退，对"已离线 + flag 仍 true"的机器（force_machine_exit 清卡死机的典型场景）会第二次
        //     update_snap_on_rent_changed(false) → era+1 EraStashPoints.total 永久多减 30%×calc_point → 分母缩水随 era
        //     滚动传播 → 全网份额和 >100% 系统性超发。故 DeepLink 分支须排除离线态（此时快照已被 machine_offline 回退）。
        //     不变量：offline_machine_change_hardware_info 对 deeplink_rented 机器直接拒绝(见该函数)，保证
        //     "DeepLink 机器处于 *ReportOffline ⟺ 快照已回退"，本处按 status 跳过才安全。
        let deeplink_snapshot_applied = Self::deeplink_rented(&machine_id)
            && !matches!(
                machine_info.machine_status,
                MachineStatus::StakerReportOffline(..) | MachineStatus::ReporterReportOffline(..)
            );
        if matches!(machine_info.machine_status, MachineStatus::Rented) || deeplink_snapshot_applied
        {
            Self::update_region_on_rent_changed(&machine_info, false);
            Self::update_snap_on_rent_changed(machine_id.clone(), false)
                .map_err(|_| Error::<T>::Unknown)?;
        }
        // [评审修 round2] 退出时清 DeepLinkRented，防孤儿 true 标记残留（机器注销后无清除入口 → 同 machine_id
        //   重注册会被守卫①永久拒原生租用 + controller_report_online 误恢复幽灵 +30%）。
        DeepLinkRented::<T>::remove(&machine_id);

        Self::update_region_on_exit(&machine_info);
        Self::update_snap_on_online_changed(machine_id.clone(), false)
            .map_err(|_| Error::<T>::Unknown)?;

        LiveMachines::<T>::mutate(|live_machines| {
            live_machines.on_exit(&machine_id);
        });

        let mut controller_machines = Self::controller_machines(&machine_info.controller);
        ItemList::rm_item(&mut controller_machines, &machine_id);
        if controller_machines.is_empty() {
            ControllerMachines::<T>::remove(&machine_info.controller);
        } else {
            ControllerMachines::<T>::insert(&machine_info.controller, controller_machines);
        }

        MachinesInfo::<T>::remove(&machine_id);
        Self::deposit_event(Event::MachineExit(machine_id));
        Ok(().into())
    }

    pub fn do_cancel_slash(slash_id: u64) -> DispatchResultWithPostInfo {
        ensure!(PendingSlash::<T>::contains_key(slash_id), Error::<T>::SlashIdNotExist);

        let slash_info = Self::pending_slash(slash_id).ok_or(Error::<T>::Unknown)?;
        let pending_slash_review =
            Self::pending_slash_review(slash_id).ok_or(Error::<T>::Unknown)?;

        Self::change_stake(&slash_info.slash_who, slash_info.slash_amount, false)
            .map_err(|_| Error::<T>::ReduceStakeFailed)?;

        Self::change_stake(&slash_info.slash_who, pending_slash_review.staked_amount, false)
            .map_err(|_| Error::<T>::ReduceStakeFailed)?;

        PendingSlashReviewChecking::<T>::mutate(
            slash_info.slash_exec_time,
            |pending_review_checking| {
                ItemList::rm_item(pending_review_checking, &slash_id);
            },
        );
        PendingExecSlash::<T>::mutate(slash_info.slash_exec_time, |pending_exec_slash| {
            ItemList::rm_item(pending_exec_slash, &slash_id);
        });

        PendingSlash::<T>::remove(slash_id);
        PendingSlashReview::<T>::remove(slash_id);

        Self::deposit_event(Event::SlashCanceled(
            slash_id,
            slash_info.slash_who,
            slash_info.slash_amount,
        ));
        Ok(().into())
    }

    /// 暂时下架机器
    fn machine_offline(
        machine_id: MachineId,
        machine_status: MachineStatus<T::BlockNumber, T::AccountId>,
    ) -> Result<(), ()> {
        let mut machine_info = Self::machines_info(&machine_id).ok_or(())?;

        LiveMachines::<T>::mutate(|live_machines| {
            live_machines.on_offline(machine_id.clone());
        });

        // 先根据机器当前状态，之后再变更成下线状态
        // [审计修 #5a] DeepLink(EVM) 租用的机器 machine_status 不是 Rented（DeepLink 不改 machine_status），
        //   原 `if Rented` 会跳过 → 离线时 total_rented_gpu/+30% 不回退、却移除点数 → 重新上线后失同步、
        //   total_rented_gpu 多计（曾现 totalRentedGpu>totalGpuNum）。补 `|| deeplink_rented` 让 DeepLink 租用
        //   也对称回退。与下方 controller_report_online 的对称恢复配套。
        if matches!(machine_info.machine_status, MachineStatus::Rented)
            || Self::deeplink_rented(&machine_id)
        {
            Self::update_region_on_rent_changed(&machine_info, false);
            Self::update_snap_on_rent_changed(machine_id.clone(), false)?;
        }

        // When offline, pos_info will be removed
        Self::update_region_on_online_changed(&machine_info, false);
        Self::update_snap_on_online_changed(machine_id.clone(), false)?;

        // After re-online, machine status is same as former
        machine_info.machine_status = machine_status;

        MachinesInfo::<T>::insert(&machine_id, machine_info);
        Ok(())
    }

    fn change_stake(who: &T::AccountId, amount: BalanceOf<T>, is_add: bool) -> Result<(), ()> {
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

        // 更改sys_info
        SysInfo::<T>::mutate(|sys_info| {
            sys_info.on_stake_changed(amount, is_add);
        });
        StashStake::<T>::insert(&who, stash_stake);

        Self::deposit_event(if is_add {
            Event::StakeAdded(who.clone(), amount)
        } else {
            Event::StakeReduced(who.clone(), amount)
        });

        Ok(())
    }

    // 获取下一Era stash grade即为当前Era stash grade
    fn get_stash_grades(era_index: EraIndex, stash: &T::AccountId) -> u64 {
        let next_era_stash_snapshot = Self::eras_stash_points(era_index);

        if let Some(stash_snapshot) = next_era_stash_snapshot.staker_statistic.get(stash) {
            stash_snapshot.total_grades().unwrap_or_default()
        } else {
            0
        }
    }

    // When Online:
    // - Writes:(currentEra) ErasStashPoints, ErasMachinePoints, SysInfo, StashMachines
    // When Offline:
    // - Writes: (currentEra) ErasStashPoints, ErasMachinePoints, (nextEra) ErasStashPoints,
    //   ErasMachinePoints SysInfo, StashMachines
    fn update_snap_on_online_changed(machine_id: MachineId, is_online: bool) -> Result<(), ()> {
        let machine_info = Self::machines_info(&machine_id).ok_or(())?;
        let machine_base_info = machine_info.machine_info_detail.committee_upload_info.clone();
        let current_era = Self::current_era();

        let mut current_era_stash_snap = Self::eras_stash_points(current_era);
        let mut next_era_stash_snap = Self::eras_stash_points(current_era + 1);
        let mut current_era_machine_snap = Self::eras_machine_points(current_era);
        let mut next_era_machine_snap = Self::eras_machine_points(current_era + 1);

        let mut stash_machine = Self::stash_machines(&machine_info.machine_stash);
        let mut sys_info = Self::sys_info();

        let pre_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);
        let current_era_is_online = current_era_machine_snap.contains_key(&machine_id);

        next_era_stash_snap.on_online_changed(
            machine_info.machine_stash.clone(),
            machine_info.gpu_num() as u64,
            machine_info.calc_point(),
            is_online,
        );

        if is_online {
            next_era_machine_snap.insert(
                machine_id.clone(),
                MachineGradeStatus { basic_grade: machine_info.calc_point(), is_rented: false },
            );
        } else if current_era_is_online {
            // NOTE: 24小时内，不能下线后再次下线。因为下线会清空当日得分记录，
            // 一天内再次下线会造成再次清空
            current_era_stash_snap.on_online_changed(
                machine_info.machine_stash.clone(),
                machine_info.gpu_num() as u64,
                machine_info.calc_point(),
                is_online,
            );
            current_era_machine_snap.remove(&machine_id);
            next_era_machine_snap.remove(&machine_id);
        }

        // 机器上线或者下线都会影响下一era得分，而只有下线才影响当前era得分
        ErasStashPoints::<T>::insert(current_era + 1, next_era_stash_snap);
        ErasMachinePoints::<T>::insert(current_era + 1, next_era_machine_snap);
        if !is_online {
            ErasStashPoints::<T>::insert(current_era, current_era_stash_snap);
            ErasMachinePoints::<T>::insert(current_era, current_era_machine_snap);
        }

        // TODO: 重新生成sys_info，因为多次调用exit时，total_gpu_num将会被调用多次
        sys_info.total_gpu_num = if is_online {
            sys_info.total_gpu_num.saturating_add(machine_base_info.gpu_num as u64)
        } else {
            sys_info.total_gpu_num.saturating_sub(machine_base_info.gpu_num as u64)
        };

        if is_online {
            ItemList::add_item(&mut stash_machine.online_machine, machine_id.clone());
            stash_machine.total_gpu_num =
                stash_machine.total_gpu_num.saturating_add(machine_base_info.gpu_num as u64);
        } else {
            ItemList::rm_item(&mut stash_machine.online_machine, &machine_id);
            stash_machine.total_gpu_num =
                stash_machine.total_gpu_num.saturating_sub(machine_base_info.gpu_num as u64);
        }

        let new_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);
        stash_machine.total_calc_points = stash_machine
            .total_calc_points
            .saturating_add(new_stash_grade)
            .saturating_sub(pre_stash_grade);

        sys_info.total_calc_points = sys_info
            .total_calc_points
            .saturating_add(new_stash_grade)
            .saturating_sub(pre_stash_grade);

        Self::adjust_rent_fee_destroy_percent(sys_info.total_gpu_num, current_era);

        if is_online && stash_machine.online_machine.len() == 1 {
            sys_info.total_staker = sys_info.total_staker.saturating_add(1);
        }
        if !is_online && stash_machine.online_machine.is_empty() {
            sys_info.total_staker = sys_info.total_staker.saturating_sub(1);
        }

        SysInfo::<T>::put(sys_info);
        StashMachines::<T>::insert(&machine_info.machine_stash, stash_machine);
        Ok(())
    }

    // - Writes:
    // ErasStashPoints, ErasMachinePoints, SysInfo, StashMachines
    /// [+30% 桥] 供 RentBridge precompile 调用：DeepLink(EVM) 租出/退租中国机器时，
    /// 标记/取消其原生挖矿 +30% 被租加成。幂等守卫防重复 toggle 破坏会计。
    /// ⚠️ 共识相关：若机器同时被原生 rent-machine 租用，会与原生 is_rented 叠加（double-count）。
    ///    用于专供 DeepLink 租用、不参与原生租用的中国机器。须经 DBC 团队评审 + 测试网验证。
    pub fn deeplink_set_rented(machine_id: MachineId, is_rented: bool) -> Result<(), ()> {
        if is_rented {
            // [互斥守卫②·原生 rent-machine] 一台机器不能同时被原生 rent-machine 与 DeepLink 租用，否则
            //   is_rented/total_rented_gpu/+30% 会计跨路径叠加 double-count。已被原生租用(MachineRentedGPU>0)→拒绝。
            //   （+30% 已由原生那次施加；EVM 侧 _notifyRentBonus try/catch 会吞此 Err，不阻塞 EVM 退租 toggle false）。
            if MachineRentedGPU::<T>::get(&machine_id) > 0 {
                return Err(());
            }
            // [互斥守卫·terminating-rental] terminating-rental 是独立租用系统、独立 +30% 会计，两边同时租同一台机
            //   会跨系统 double-count。链上强制互斥（不靠运营约定）：该机在 terminating-rental 有活跃租用→拒绝上 DeepLink 租。
            if T::TerminatingRentalStatus::is_machine_rented(&machine_id) {
                return Err(());
            }
            // [在线前置校验] 只在真实 false->true 转换时要求机器 == Online（对齐原生 rent 可租前置；收窄为仅
            //   Online——原生 Online||Rented 里的 Rented 已被守卫②拒）。幂等 no-op(已 true) 不重复校验；
            //   退租(false)永不校验（DeepLink 租用期间机器可能掉线，退租必须永远能清标记，见 apply 的离线感知）。
            if !Self::deeplink_rented(&machine_id) {
                let is_online = matches!(
                    Self::machines_info(&machine_id).map(|mi| mi.machine_status),
                    Some(MachineStatus::Online)
                );
                if !is_online {
                    return Err(());
                }
            }
        }
        Self::apply_deeplink_rented(&machine_id, is_rented)
    }

    /// 内部：落 DeepLink 租用标记 + **离线/注销感知**的快照对账（进出两个方向都感知）。
    /// [评审修 round2] is_rented=false 时若机器已离线（快照已被 machine_offline 在下线时回退）或已注销（无 era
    ///   快照），**跳过** update_snap_on_rent_changed、只落标记——否则会 double-减 total_rented_gpu/stash grade
    ///   (+30% 误扣同 stash 其它在租机器奖励)。对齐原生 change_machine_status_on_rent_end(traits.rs)：离线分支
    ///   只记 RentedFinished、绝不回退快照。重新上线由 controller_report_online 的 deeplink_rented 分支对称恢复。
    /// [审计修 round3] **is_rented=true 时同样感知离线**：若机器当前离线，EVM 侧此刻 setRented(true) 不加快照、
    ///   只落标记；等 controller_report_online 的 deeplink_rented 分支上线时把 +30%/total_rented_gpu 施加**恰好一次**。
    ///   否则（旧逻辑 true 路径恒 apply）会与上线恢复分支叠加，出现 total_rented_gpu += 2*gpu_num 的 double-count，
    ///   触发 totalRentedGpu>totalGpuNum。注销(None)+true 保持 skip_snap=false → update_snap 因 machines_info 缺失 Err
    ///   → 拒绝给不存在的机器落标记（不造幽灵条目）。
    /// 幂等：标记未变则 no-op。机器注销时 false 路径也 Ok（顺带让 EVM endRent 自愈清标记，不再 Err 卡死）。
    fn apply_deeplink_rented(machine_id: &MachineId, is_rented: bool) -> Result<(), ()> {
        if Self::deeplink_rented(machine_id) == is_rented {
            return Ok(());
        }
        let skip_snap = match Self::machines_info(machine_id) {
            Some(mi) => matches!(
                mi.machine_status,
                MachineStatus::StakerReportOffline(..) | MachineStatus::ReporterReportOffline(..)
            ),
            // 注销：false 无快照可回退 → skip；true 保持 false 让 update_snap Err 拒绝幽灵租用。
            None => !is_rented,
        };
        if !skip_snap {
            Self::update_snap_on_rent_changed(machine_id.clone(), is_rented)?;
        }
        DeepLinkRented::<T>::insert(machine_id, is_rented);
        Ok(())
    }

    fn update_snap_on_rent_changed(machine_id: MachineId, is_rented: bool) -> Result<(), ()> {
        let machine_info = Self::machines_info(&machine_id).ok_or(())?;
        let current_era = Self::current_era();

        let mut current_era_stash_snap = Self::eras_stash_points(current_era);
        let mut next_era_stash_snap = Self::eras_stash_points(current_era + 1);
        let mut current_era_machine_snap = Self::eras_machine_points(current_era);
        let mut next_era_machine_snap = Self::eras_machine_points(current_era + 1);

        let mut stash_machine = Self::stash_machines(&machine_info.machine_stash);
        let mut sys_info = Self::sys_info();

        let current_era_is_online = current_era_machine_snap.contains_key(&machine_id);
        let current_era_is_rented = if current_era_is_online {
            let machine_snap = current_era_machine_snap.get(&machine_id).unwrap();
            machine_snap.is_rented
        } else {
            false
        };

        let pre_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);

        next_era_stash_snap.on_rent_changed(
            machine_info.machine_stash.clone(),
            machine_info.calc_point(),
            is_rented,
        );
        next_era_machine_snap.insert(
            machine_id.clone(),
            MachineGradeStatus { basic_grade: machine_info.calc_point(), is_rented },
        );

        if !is_rented {
            if current_era_is_rented {
                current_era_stash_snap.on_rent_changed(
                    machine_info.machine_stash.clone(),
                    machine_info.calc_point(),
                    is_rented,
                );
            }

            current_era_machine_snap.insert(
                machine_id,
                MachineGradeStatus { basic_grade: machine_info.calc_point(), is_rented },
            );
        }

        // 被租用或者退租都影响下一Era记录，而退租直接影响当前得分
        ErasStashPoints::<T>::insert(current_era + 1, next_era_stash_snap);
        ErasMachinePoints::<T>::insert(current_era + 1, next_era_machine_snap);
        if !is_rented {
            ErasStashPoints::<T>::insert(current_era, current_era_stash_snap);
            ErasMachinePoints::<T>::insert(current_era, current_era_machine_snap);
        }

        let gpu_num = machine_info.gpu_num() as u64;

        sys_info.total_rented_gpu = if is_rented {
            sys_info.total_rented_gpu.saturating_add(gpu_num)
        } else {
            sys_info.total_rented_gpu.saturating_sub(gpu_num)
        };
        stash_machine.total_rented_gpu = if is_rented {
            stash_machine.total_rented_gpu.saturating_add(gpu_num)
        } else {
            stash_machine.total_rented_gpu.saturating_sub(gpu_num)
        };

        let new_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);
        stash_machine.total_calc_points = stash_machine
            .total_calc_points
            .saturating_add(new_stash_grade)
            .saturating_sub(pre_stash_grade);
        sys_info.total_calc_points = sys_info
            .total_calc_points
            .saturating_add(new_stash_grade)
            .saturating_sub(pre_stash_grade);

        SysInfo::<T>::put(sys_info);
        StashMachines::<T>::insert(&machine_info.machine_stash, stash_machine);
        Ok(())
    }

    fn adjust_rent_fee_destroy_percent(gpu_num: u64, current_era: u32) {
        // NOTE: 5000张卡开启银河竞赛: 奖励增加
        if gpu_num == 5000 {
            let mut phase_reward_info = Self::phase_reward_info().unwrap_or_default();
            if phase_reward_info.galaxy_on_era == 0 {
                phase_reward_info.galaxy_on_era = current_era;
                PhaseRewardInfo::<T>::put(phase_reward_info);
            }
        }

        // 租金销毁比例固定5%，不再根据GPU数量动态调整
        // 可通过 sudo 调用 set_rentfee_destroy_percent 手动修改
    }

    // 当租金转给该stash账户，或者领取在线奖励后，会检查机器奖励是否足够
    // 如果不够，则会按顺序补充机器质押
    fn fulfill_machine_stake(stash: T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        let mut amount_left = amount;

        let stash_machines = Self::stash_machines(&stash);
        for machine_id in stash_machines.online_machine.iter() {
            let mut machine_info = match Self::machines_info(&machine_id) {
                Some(machine_info) => machine_info,
                None => continue,
            };

            let stake_amount_per_gpu = Self::stake_per_gpu().ok_or(())?;
            let stake_need = stake_amount_per_gpu
                .checked_mul(&machine_info.gpu_num().saturated_into::<BalanceOf<T>>())
                .ok_or(())?;

            if stake_need <= machine_info.stake_amount {
                continue
            }
            // 现在需要的stake 比 已经stake的多了。
            let extra_need = stake_need - machine_info.stake_amount; // 这个机器还需要这么多质押。
            let pre_stake = machine_info.stake_amount;

            if extra_need <= amount_left {
                // best-effort：若 reserve 失败，停止补质押（保留已完成的部分，不返回 Err，
                // 避免在非回滚调用方处留下半完成状态）。
                if Self::change_stake(&machine_info.machine_stash, extra_need, true).is_err() {
                    break
                }
                amount_left = amount_left.saturating_sub(extra_need);
                machine_info.stake_amount = stake_need;

                MachinesInfo::<T>::insert(&machine_id, machine_info);
                Self::deposit_event(Event::MachineAddStake(
                    machine_id.clone(),
                    pre_stake,
                    extra_need,
                ));
            } else {
                if Self::change_stake(&machine_info.machine_stash, amount_left, true).is_err() {
                    return Ok(())
                }
                machine_info.stake_amount = machine_info.stake_amount.saturating_add(amount_left);
                MachinesInfo::<T>::insert(&machine_id, machine_info);
                Self::deposit_event(Event::MachineAddStake(
                    machine_id.clone(),
                    pre_stake,
                    amount_left,
                ));
                return Ok(())
            }
        }
        Ok(())
    }

    pub fn add_offline_machine_to_renters(machine_id: MachineId, renters: Vec<T::AccountId>) {
        OfflineMachine2renters::<T>::mutate(machine_id, |renters_exists| *renters_exists = renters);
    }
}
