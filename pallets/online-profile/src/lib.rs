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

use dbc_support::{
    live_machine::LiveMachine,
    machine_info::MachineInfo,
    machine_type::{Latitude, Longitude, MachineStatus, StakerCustomizeInfo},
    traits::{DbcPrice, GNOps, ManageCommittee},
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
    SaturatedConversion,
};
use sp_std::{
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    convert::From,
    prelude::*,
    str,
    vec::Vec,
};

pub use pallet::*;
pub use traits::*;
pub use types::*;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + generic_func::Config {
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
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    #[pallet::without_storage_info]
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

    /// If galaxy competition is begin: switch 5000 gpu
    #[pallet::storage]
    #[pallet::getter(fn galaxy_is_on)]
    pub(super) type GalaxyIsOn<T: Config> = StorageValue<_, bool, ValueQuery>;

    #[pallet::type_value]
    pub(super) fn GalaxyOnGPUThresholdDefault<T: Config>() -> u32 {
        5000
    }

    #[pallet::storage]
    #[pallet::getter(fn galaxy_on_gpu_threshold)]
    pub(super) type GalaxyOnGPUThreshold<T: Config> =
        StorageValue<_, u32, ValueQuery, GalaxyOnGPUThresholdDefault<T>>;

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

    /// 2880 Block/Era
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

    /// 将要发放奖励的机器
    #[pallet::storage]
    #[pallet::getter(fn all_machine_id_snap)]
    pub(super) type AllMachineIdSnap<T: Config> =
        StorageValue<_, types::AllMachineIdSnapDetail, ValueQuery>;

    /// 资金账户的质押总计
    #[pallet::storage]
    #[pallet::getter(fn stash_stake)]
    pub(super) type StashStake<T: Config> =
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

    // The current storage version.
    #[pallet::storage]
    #[pallet::getter(fn storage_version)]
    pub(super) type StorageVersion<T: Config> = StorageValue<_, u16, ValueQuery>;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(block_number: T::BlockNumber) -> Weight {
            Self::backup_and_reward(block_number);

            if block_number.saturated_into::<u64>() % (ONE_DAY as u64) == 1 {
                // Era开始时，生成当前Era和下一个Era的快照
                // 每个Era(2880个块)执行一次
                Self::update_snap_for_new_era();
            }
            Self::exec_pending_slash();
            let _ = Self::check_pending_slash();
            Weight::zero()
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// When reward start to distribute
        #[pallet::call_index(0)]
        #[pallet::weight(0)]
        pub fn set_reward_info(
            origin: OriginFor<T>,
            reward_info: PhaseRewardInfoDetail<BalanceOf<T>>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            <PhaseRewardInfo<T>>::put(reward_info);
            Ok(().into())
        }

        #[pallet::call_index(1)]
        #[pallet::weight(0)]
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
        #[pallet::weight(0)]
        pub fn set_standard_gpu_point_price(
            origin: OriginFor<T>,
            point_price: dbc_support::machine_type::StandardGpuPointPrice,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StandardGPUPointPrice::<T>::put(point_price);
            Ok(().into())
        }

        #[pallet::call_index(3)]
        #[pallet::weight(0)]
        pub fn set_galaxy_on(origin: OriginFor<T>, is_on: bool) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            GalaxyIsOn::<T>::put(is_on);
            Ok(().into())
        }

        #[pallet::call_index(4)]
        #[pallet::weight(0)]
        pub fn set_galaxy_on_gpu_threshold(
            origin: OriginFor<T>,
            gpu_threshold: u32,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            GalaxyOnGPUThreshold::<T>::put(gpu_threshold);

            let mut phase_reward_info = Self::phase_reward_info().unwrap_or_default();
            let current_era = Self::current_era();
            let sys_info = Self::sys_info();

            // NOTE: 5000张卡开启银河竞赛
            if !Self::galaxy_is_on() && sys_info.total_gpu_num >= gpu_threshold as u64 {
                phase_reward_info.galaxy_on_era = current_era;
                PhaseRewardInfo::<T>::put(phase_reward_info);
                GalaxyIsOn::<T>::put(true);
            }

            Ok(().into())
        }

        /// Stash account set a controller
        #[pallet::call_index(5)]
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

        // - Writes: controller_machines, stash_controller, controller_stash, machine_info,
        /// Stash account reset controller for one machine
        #[pallet::call_index(6)]
        #[pallet::weight(10000)]
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
        #[pallet::call_index(7)]
        #[pallet::weight(10000)]
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

            // 重新上链需要质押一定的手续费
            let online_stake_params =
                Self::online_stake_params().ok_or(Error::<T>::GetReonlineStakeFailed)?;
            let stake_amount =
                T::DbcPrice::get_dbc_amount_by_value(online_stake_params.reonline_stake)
                    .ok_or(Error::<T>::GetReonlineStakeFailed)?;

            machine_info.machine_status =
                MachineStatus::StakerReportOffline(now, Box::new(MachineStatus::Online));

            Self::change_stake(&machine_info.machine_stash, stake_amount, true)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;
            UserMutHardwareStake::<T>::insert(
                &machine_info.machine_stash,
                &machine_id,
                UserMutHardwareStakeInfo { stake_amount, offline_time: now },
            );
            Self::update_region_on_online_changed(&machine_info, false);
            Self::update_snap_on_online_changed(machine_id.clone(), false)
                .map_err(|_| Error::<T>::Unknown)?;

            LiveMachines::<T>::mutate(|live_machines| {
                live_machines.on_offline_change_hardware(machine_id.clone());
            });
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::MachineOfflineToMutHardware(machine_id, stake_amount));
            Ok(().into())
        }

        /// Controller account submit online request machine
        #[pallet::call_index(8)]
        #[pallet::weight(10000)]
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
            let stake_amount = Self::stake_per_gpu().ok_or(Error::<T>::CalcStakeAmountFailed)?;
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
        #[pallet::call_index(9)]
        #[pallet::weight(10000)]
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
        #[pallet::call_index(10)]
        #[pallet::weight(10000)]
        pub fn add_machine_info(
            origin: OriginFor<T>,
            machine_id: MachineId,
            server_room_info: StakerCustomizeInfo,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            // 查询机器Id是否在该账户的控制下
            let machine_info = Self::machines_info(&machine_id).ok_or(Error::<T>::Unknown)?;
            machine_info
                .can_add_server_room(&controller)
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
                Ok::<(), sp_runtime::DispatchError>(())
            })?;
            // 当是第一次上线添加机房信息时
            LiveMachines::<T>::mutate(|live_machines| {
                live_machines
                    .on_add_server_room(machine_id.clone(), machine_info.machine_status.clone())
            });

            Self::deposit_event(Event::MachineInfoAdded(machine_id));
            Ok(().into())
        }

        // 机器第一次上线后处于补交质押状态时
        // 或者机器更改配置信息后，处于质押不足状态时, 需要补交质押才能上线
        #[pallet::call_index(11)]
        #[pallet::weight(10000)]
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

            if <UserMutHardwareStake<T>>::contains_key(&machine_info.machine_stash, &machine_id) {
                // 根据质押，奖励给这些委员会
                let reonline_stake =
                    Self::user_mut_hardware_stake(&machine_info.machine_stash, &machine_id);

                // 根据下线时间，惩罚stash
                let offline_duration = now.saturating_sub(reonline_stake.offline_time);
                // 如果下线的时候空闲超过10天，则不进行惩罚
                if reonline_stake.offline_time < machine_info.last_online_height + 28800u32.into() {
                    // 记录该惩罚数据
                    let slash_info = Self::new_slash_when_offline(
                        machine_id.clone(),
                        OPSlashReason::OnlineReportOffline(reonline_stake.offline_time),
                        None,
                        vec![],
                        None,
                        offline_duration,
                    )
                    .map_err(|_| Error::<T>::Unknown)?;
                    let slash_id = Self::get_new_slash_id();

                    PendingExecSlash::<T>::mutate(
                        slash_info.slash_exec_time,
                        |pending_exec_slash| {
                            ItemList::add_item(pending_exec_slash, slash_id);
                        },
                    );
                    PendingSlash::<T>::insert(slash_id, slash_info);
                }
                // 退还reonline_stake
                Self::change_stake(&machine_info.machine_stash, reonline_stake.stake_amount, false)
                    .map_err(|_| Error::<T>::ReduceStakeFailed)?;
                UserMutHardwareStake::<T>::remove(&machine_info.machine_stash, &machine_id);
            } else {
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
            }

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
        #[pallet::call_index(12)]
        #[pallet::weight(10000)]
        pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashAccount)?;

            ensure!(StashMachines::<T>::contains_key(&stash), Error::<T>::NotMachineController);

            StashMachines::<T>::mutate(&stash, |stash_machine| -> DispatchResultWithPostInfo {
                let can_claim =
                    stash_machine.claim_reward().map_err(|_| Error::<T>::ClaimRewardFailed)?;

                <T as Config>::Currency::deposit_into_existing(&stash, can_claim)
                    .map_err(|_| Error::<T>::ClaimRewardFailed)?;
                Self::deposit_event(Event::ClaimReward(stash.clone(), can_claim));
                Ok(().into())
            })
        }

        /// 控制账户报告机器下线:Online/Rented时允许
        #[pallet::call_index(13)]
        #[pallet::weight(10000)]
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

            Self::machine_offline(
                machine_id.clone(),
                MachineStatus::StakerReportOffline(now, Box::new(machine_info.machine_status)),
            )
            .map_err(|_| Error::<T>::Unknown)?;

            Self::deposit_event(Event::ControllerReportOffline(machine_id));
            Ok(().into())
        }

        // NOTE: 如果机器主动下线/因举报下线之后，几个租用订单陆续到期，则机器主动上线
        // 要根据几个订单的状态来判断机器是否是在线/租用状态
        // 需要在rentMachine中提供一个查询接口
        /// 控制账户报告机器上线
        #[pallet::call_index(14)]
        #[pallet::weight(10000)]
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
                        machine_info.renters.clone(),
                        Some(committee),
                        offline_duration,
                    )
                },
                _ => return Err(Error::<T>::MachineStatusNotAllowed.into()),
            }
            .map_err(|_| Error::<T>::Unknown)?;

            // NOTE: 如果机器上线超过一年，空闲超过10天，下线后上线不添加惩罚
            if now >= machine_info.online_height &&
                now.saturating_sub(machine_info.online_height) > (365 * 2880u32).into() &&
                offline_time >= machine_info.last_online_height &&
                offline_time.saturating_sub(machine_info.last_online_height) >=
                    (10 * 2880u32).into() &&
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
        #[pallet::call_index(15)]
        #[pallet::weight(10000)]
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
                now.saturating_sub(machine_info.last_online_height) >= 28800u32.into(),
                Error::<T>::TimeNotAllowed
            );

            Self::do_machine_exit(machine_id, machine_info)
        }

        /// 满足365天可以申请重新质押，退回质押币
        /// 在系统中上线满365天之后，可以按当时机器需要的质押数量，重新入网。多余的币解绑
        /// 在重新上线之后，下次再执行本操作，需要等待365天
        #[pallet::call_index(16)]
        #[pallet::weight(10000)]
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
            ensure!(machine_info.stake_amount > stake_need, Error::<T>::NoStakeToReduce);

            let extra_stake = machine_info
                .stake_amount
                .checked_sub(&stake_need)
                .ok_or(Error::<T>::ReduceStakeFailed)?;

            machine_info.stake_amount = stake_need;
            machine_info.last_machine_restake = now;
            machine_info.init_stake_per_gpu = stake_per_gpu;
            Self::change_stake(&machine_info.machine_stash, extra_stake, false)
                .map_err(|_| Error::<T>::ReduceStakeFailed)?;

            MachinesInfo::<T>::insert(&machine_id, machine_info.clone());

            Self::deposit_event(Event::MachineRestaked(machine_id, pre_stake, stake_need));
            Ok(().into())
        }

        #[pallet::call_index(17)]
        #[pallet::weight(10000)]
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

        #[pallet::call_index(18)]
        #[pallet::weight(0)]
        pub fn cancel_slash(origin: OriginFor<T>, slash_id: u64) -> DispatchResultWithPostInfo {
            T::CancelSlashOrigin::ensure_origin(origin)?;
            Self::do_cancel_slash(slash_id)
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
        MachineOfflineToMutHardware(MachineId, BalanceOf<T>),
        StakeAdded(T::AccountId, BalanceOf<T>),
        StakeReduced(T::AccountId, BalanceOf<T>),
        ServerRoomGenerated(T::AccountId, H256),
        MachineInfoAdded(MachineId),
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
    }

    #[pallet::error]
    pub enum Error<T> {
        BadSignature,
        MachineIdExist,
        BalanceNotEnough,
        NotMachineController,
        PayTxFeeFailed,
        RewardPhaseOutOfRange,
        ClaimRewardFailed,
        ConvertMachineIdToWalletFailed,
        NoStashBond,
        AlreadyController,
        NoStashAccount,
        BadMsgLen,
        NotAllowedChangeMachineInfo,
        MachineStashNotEqualControllerStash,
        CalcStakeAmountFailed,
        NotRefusedMachine,
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
    }
}

impl<T: Config> Pallet<T> {
    // NOTE: StashMachine.total_machine cannot be removed. Because Machine will be rewarded in 150 eras.
    pub fn do_machine_exit(
        machine_id: MachineId,
        machine_info: MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
    ) -> DispatchResultWithPostInfo {
        // 下线机器，并退还奖励
        Self::change_stake(&machine_info.machine_stash, machine_info.stake_amount, false)
            .map_err(|_| Error::<T>::ReduceStakeFailed)?;

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
        if matches!(machine_info.machine_status, MachineStatus::Rented) {
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

        // NOTE: 5000张卡开启银河竞赛
        if !Self::galaxy_is_on() && sys_info.total_gpu_num >= Self::galaxy_on_gpu_threshold() as u64
        {
            let mut phase_reward_info = Self::phase_reward_info().unwrap_or_default();
            phase_reward_info.galaxy_on_era = current_era;
            PhaseRewardInfo::<T>::put(phase_reward_info);
            GalaxyIsOn::<T>::put(true);
        }

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
}
