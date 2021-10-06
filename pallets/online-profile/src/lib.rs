#![cfg_attr(not(feature = "std"), no_std)]

use codec::EncodeLike;
use frame_support::{
    dispatch::DispatchResultWithPostInfo,
    pallet_prelude::*,
    traits::{Currency, EnsureOrigin, ExistenceRequirement::KeepAlive, Get, OnUnbalanced, ReservableCurrency},
    weights::Weight,
    IterableStorageDoubleMap, IterableStorageMap,
};
use frame_system::pallet_prelude::*;
use generic_func::ItemList;
use online_profile_machine::{DbcPrice, GNOps, MTOps, ManageCommittee, OCOps, OPRPCQuery, RTOps};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::{crypto::Public, H256};
use sp_runtime::{
    traits::{CheckedAdd, CheckedMul, CheckedSub, Verify, Zero},
    Perbill, SaturatedConversion,
};
use sp_std::{
    collections::btree_map::BTreeMap,
    convert::{From, TryFrom, TryInto},
    prelude::*,
    str,
    vec::Vec,
};

pub mod op_types;
pub mod rpc_types;

pub use op_types::*;
pub use rpc_types::*;

pub use pallet::*;

/// 2880 blocks per era
pub const BLOCK_PER_ERA: u64 = 2880;
/// Reward duration for committee (Era)
pub const REWARD_DURATION: u32 = 365 * 2;
/// Rebond frequency, 1 year
pub const REBOND_FREQUENCY: u32 = 365 * 2880;
/// Max Slash Threshold: 120h, 5 era
pub const MAX_SLASH_THRESHOLD: u32 = 2880 * 5;

/// stash account overview self-status
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct StashMachine<Balance> {
    /// All machines bonded to stash account, if machine is offline,
    /// rm from this field after 150 Eras for linear release
    pub total_machine: Vec<MachineId>,
    /// Machines, that is in passed committee verification
    pub online_machine: Vec<MachineId>,
    /// Total grades of all online machine, inflation(for multiple GPU of one stash / reward by rent) is counted
    pub total_calc_points: u64,
    /// Total online gpu num, will be added after online, reduced after offline
    pub total_gpu_num: u64,
    /// Total rented gpu
    pub total_rented_gpu: u64,
    /// All reward stash account got, locked reward included
    pub total_earned_reward: Balance,
    /// Sum of all claimed reward
    pub total_claimed_reward: Balance,
    /// Reward can be claimed now
    pub can_claim_reward: Balance,
    /// How much has been earned by rent before Galaxy is on
    pub total_rent_fee: Balance,
    /// How much has been burned after Galaxy is on
    pub total_burn_fee: Balance,
}

/// All details of a machine
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct MachineInfo<AccountId: Ord, BlockNumber, Balance> {
    /// Who can control this machine
    pub controller: AccountId,
    /// Who own this machine and will get machine's reward
    pub machine_stash: AccountId,
    /// Last machine renter
    pub last_machine_renter: Option<AccountId>,
    /// Every 365 days machine can restake(For token price maybe changed)
    pub last_machine_restake: BlockNumber,
    /// When controller bond this machine
    pub bonding_height: BlockNumber,
    /// When machine is passed verification and is online
    pub online_height: BlockNumber,
    /// Last time machine is online
    /// (When first online; Rented -> Online, Offline -> Online e.t.)
    pub last_online_height: BlockNumber,
    /// When first bond_machine, record how much should stake per GPU
    pub init_stake_per_gpu: Balance,
    /// How much machine staked
    pub stake_amount: Balance,
    /// Status of machine
    pub machine_status: MachineStatus<BlockNumber, AccountId>,
    /// How long machine has been rented(will be update after one rent is end)
    pub total_rented_duration: u64,
    /// How many times machine has been rented
    pub total_rented_times: u64,
    /// How much rent fee machine has earned for rented(before Galaxy is ON)
    pub total_rent_fee: Balance,
    /// How much rent fee is burn after Galaxy is ON
    pub total_burn_fee: Balance,
    /// Machine's hardware info
    pub machine_info_detail: MachineInfoDetail,
    /// Committees, verified machine and will be rewarded in the following days.
    /// (In next 2 years after machine is online, get 1% unlocked reward)
    pub reward_committee: Vec<AccountId>,
    /// When reward will be over for committees
    pub reward_deadline: EraIndex,
}

/// All kind of status of a machine
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum MachineStatus<BlockNumber, AccountId> {
    /// After controller bond machine; means waiting for submit machine info
    AddingCustomizeInfo,
    /// After submit machine info; will waiting to distribute order to committees
    DistributingOrder,
    /// After distribute to committees, should take time to verify hardware
    CommitteeVerifying,
    /// Machine is refused by committees, so cannot be online
    CommitteeRefused(BlockNumber),
    /// After committee agree machine online, stake should be paied depend on gpu num
    WaitingFulfill,
    /// Machine online successfully
    Online,
    /// Controller offline machine
    StakerReportOffline(BlockNumber, Box<Self>),
    /// Reporter report machine is fault, so machine go offline (SlashReason, StatusBeforeOffline, Reporter, Committee)
    ReporterReportOffline(OPSlashReason<BlockNumber>, Box<Self>, AccountId, Vec<AccountId>),
    /// Machine is rented, and waiting for renter to confirm virtual machine is created successfully
    Creating,
    /// Machine is rented now
    Rented,
    /// Machine is exit
    Exit,
}

impl<BlockNumber, AccountId> Default for MachineStatus<BlockNumber, AccountId> {
    fn default() -> Self {
        MachineStatus::AddingCustomizeInfo
    }
}

/// The reason why a stash account is punished
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum OPSlashReason<BlockNumber> {
    /// Controller report rented machine offline
    RentedReportOffline(BlockNumber),
    /// Controller report online machine offline
    OnlineReportOffline(BlockNumber),
    /// Reporter report rented machine is offline
    RentedInaccessible(BlockNumber),
    /// Reporter report rented machine hardware fault
    RentedHardwareMalfunction(BlockNumber),
    /// Reporter report rented machine is fake
    RentedHardwareCounterfeit(BlockNumber),
    /// Machine is online, but rent failed
    OnlineRentFailed(BlockNumber),
    /// Committee refuse machine online
    CommitteeRefusedOnline,
    /// Committee refuse changed hardware info machine reonline
    CommitteeRefusedMutHardware,
    /// Machine change hardware is passed, so should reward committee
    ReonlineShouldReward,
}

impl<BlockNumber> Default for OPSlashReason<BlockNumber> {
    fn default() -> Self {
        Self::CommitteeRefusedOnline
    }
}

/// MachineList in online module
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct LiveMachine {
    /// After call bond_machine, machine is stored waitting for controller add info
    pub bonding_machine: Vec<MachineId>,
    /// Machines, have added info, waiting for distributing to committee
    pub confirmed_machine: Vec<MachineId>,
    /// Machines, have booked by committees
    pub booked_machine: Vec<MachineId>,
    /// Verified by committees, and is online to get rewrad
    pub online_machine: Vec<MachineId>,
    /// Verified by committees, but stake is not enough:
    /// One gpu is staked first time call bond_machine, after committee verification,
    /// actual stake is calced by actual gpu num
    pub fulfilling_machine: Vec<MachineId>,
    /// Machines, refused by committee
    pub refused_machine: Vec<MachineId>,
    /// Machines, is rented
    pub rented_machine: Vec<MachineId>,
    /// Machines, called offline by controller
    pub offline_machine: Vec<MachineId>,
    /// Machines, want to change hardware info, but refused by committee
    pub refused_mut_hardware_machine: Vec<MachineId>,
}

impl LiveMachine {
    /// Check if machine_id exist
    fn machine_id_exist(&self, machine_id: &MachineId) -> bool {
        self.bonding_machine.binary_search(machine_id).is_ok() ||
            self.confirmed_machine.binary_search(machine_id).is_ok() ||
            self.booked_machine.binary_search(machine_id).is_ok() ||
            self.online_machine.binary_search(machine_id).is_ok() ||
            self.fulfilling_machine.binary_search(machine_id).is_ok() ||
            self.refused_machine.binary_search(machine_id).is_ok() ||
            self.rented_machine.binary_search(machine_id).is_ok() ||
            self.offline_machine.binary_search(machine_id).is_ok() ||
            self.refused_mut_hardware_machine.binary_search(machine_id).is_ok()
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OnlineStakeParamsInfo<Balance> {
    /// How much a GPU should stake(DBC).eg. 100_000 DBC
    pub online_stake_per_gpu: Balance,
    /// Limit of value of one GPU's actual stake。USD*10^6
    pub online_stake_usd_limit: u64,
    /// How much should stake when want reonline (change hardware info). USD*10^6
    pub reonline_stake: u64,
}

/// Standard GPU rent price Per Era
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct StandardGpuPointPrice {
    /// Standard GPU calc points
    pub gpu_point: u64,
    /// Standard GPU price
    pub gpu_price: u64,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct UserReonlineStakeInfo<Balance, BlockNumber> {
    pub stake_amount: Balance,
    pub offline_time: BlockNumber,
}

// 365 day per year
// Testnet start from 2021-07-18, after 3 years(365*3), in 2024-07-17, phase 1 should end.
// If galxy is on, Reward is double in 60 eras. So, phase 1 should end in 2024-05-18 (365*3-60)
// So, **first_phase_duration** should equal: 365 * 3 - 60 - (online_day - 2021-0718)
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct PhaseRewardInfoDetail<Balance> {
    pub online_reward_start_era: EraIndex, // When online reward will start
    pub first_phase_duration: EraIndex,
    pub galaxy_on_era: EraIndex,         // When galaxy is on
    pub phase_0_reward_per_era: Balance, // first 3 years
    pub phase_1_reward_per_era: Balance, // next 5 years
    pub phase_2_reward_per_era: Balance, // next 5 years
}

type BalanceOf<T> = <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

/// SysInfo of onlineProfile pallet
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct SysInfoDetail<Balance> {
    /// Total online gpu
    pub total_gpu_num: u64,
    /// Total rented gpu
    pub total_rented_gpu: u64,
    /// Total stash number (at lease one gpu is online)
    pub total_staker: u64,
    /// Total calc points of all gpu. (Extra rewarded grades is counted)
    pub total_calc_points: u64,
    /// Total stake of all stash account
    pub total_stake: Balance,
    /// Total rent fee before Galaxy is on
    pub total_rent_fee: Balance,
    /// Total burn fee (after Galaxy is on, rent fee will burn)
    pub total_burn_fee: Balance,
}

/// Statistics of gpus based on position(latitude and longitude)
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct PosInfo {
    /// Online gpu num in one position
    pub online_gpu: u64,
    /// Offline gpu num in one position
    pub offline_gpu: u64,
    /// Rented gpu num in one position
    pub rented_gpu: u64,
    /// Online gpu grades (NOTE: Extra rewarded grades is not counted)
    pub online_gpu_calc_points: u64,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OPPendingSlashInfo<AccountId, BlockNumber, Balance> {
    /// Who will be slashed
    pub slash_who: AccountId,
    /// Which machine will be slashed
    pub machine_id: MachineId,
    /// When slash action is created(not exec time)
    pub slash_time: BlockNumber,
    /// How much slash will be
    pub slash_amount: Balance,
    /// When slash will be exec
    pub slash_exec_time: BlockNumber,
    /// If reporter is some, will be rewarded when slash is executed
    pub reward_to_reporter: Option<AccountId>,
    /// If committee is some, will be rewarded when slash is executed
    pub reward_to_committee: Option<Vec<AccountId>>,
    /// Why one is slashed
    pub slash_reason: OPSlashReason<BlockNumber>,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + dbc_price_ocw::Config + generic_func::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type BondingDuration: Get<EraIndex>;
        type DbcPrice: DbcPrice<BalanceOf = BalanceOf<Self>>;
        type ManageCommittee: ManageCommittee<AccountId = Self::AccountId, BalanceOf = BalanceOf<Self>>;
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
        type CancelSlashOrigin: EnsureOrigin<Self::Origin>;
        type SlashAndReward: GNOps<AccountId = Self::AccountId, BalanceOf = BalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn online_stake_params)]
    pub(super) type OnlineStakeParams<T: Config> = StorageValue<_, OnlineStakeParamsInfo<BalanceOf<T>>>;

    /// A standard example for rent fee calculation(price: USD*10^6)
    #[pallet::storage]
    #[pallet::getter(fn standard_gpu_point_price)]
    pub(super) type StandardGPUPointPrice<T: Config> = StorageValue<_, StandardGpuPointPrice>;

    /// Reonline to change hardware, should stake some balance
    #[pallet::storage]
    #[pallet::getter(fn user_reonline_stake)]
    pub(super) type UserReonlineStake<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        MachineId,
        UserReonlineStakeInfo<BalanceOf<T>, T::BlockNumber>,
        ValueQuery,
    >;

    /// If galaxy competition is begin: switch 5000 gpu
    #[pallet::storage]
    #[pallet::getter(fn galaxy_is_on)]
    pub(super) type GalaxyIsOn<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// Statistics of gpu and stake
    #[pallet::storage]
    #[pallet::getter(fn sys_info)]
    pub(super) type SysInfo<T: Config> = StorageValue<_, SysInfoDetail<BalanceOf<T>>, ValueQuery>;

    /// Statistics of gpu in one position
    #[pallet::storage]
    #[pallet::getter(fn pos_gpu_info)]
    pub(super) type PosGPUInfo<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, Longitude, Blake2_128Concat, Latitude, PosInfo, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn stash_controller)]
    pub(super) type StashController<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    #[pallet::storage]
    #[pallet::getter(fn controller_stash)]
    pub(super) type ControllerStash<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    /// Detail info of machines
    #[pallet::storage]
    #[pallet::getter(fn machines_info)]
    pub type MachinesInfo<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>, ValueQuery>;

    /// Statistics of stash account
    #[pallet::storage]
    #[pallet::getter(fn stash_machines)]
    pub(super) type StashMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, StashMachine<BalanceOf<T>>, ValueQuery>;

    /// Server rooms in stash account
    #[pallet::storage]
    #[pallet::getter(fn stash_server_rooms)]
    pub(super) type StashServerRooms<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Vec<H256>, ValueQuery>;

    /// All machines in one server room
    #[pallet::storage]
    #[pallet::getter(fn server_room_machines)]
    pub(super) type ServerRoomMachines<T: Config> = StorageMap<_, Blake2_128Concat, H256, Vec<MachineId>>;

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
        StorageMap<_, Blake2_128Concat, EraIndex, EraStashPoints<T::AccountId>>;

    /// 每个Era机器的得分快照
    #[pallet::storage]
    #[pallet::getter(fn eras_machine_points)]
    pub(super) type ErasMachinePoints<T: Config> =
        StorageMap<_, Blake2_128Concat, EraIndex, BTreeMap<MachineId, MachineGradeStatus>>;

    #[pallet::storage]
    #[pallet::getter(fn phase_reward_info)]
    pub(super) type PhaseRewardInfo<T: Config> = StorageValue<_, PhaseRewardInfoDetail<BalanceOf<T>>>;

    #[pallet::storage]
    #[pallet::getter(fn era_reward)]
    pub(super) type EraReward<T: Config> = StorageMap<_, Blake2_128Concat, EraIndex, BalanceOf<T>, ValueQuery>;

    /// 某个Era机器获得的总奖励
    #[pallet::storage]
    #[pallet::getter(fn eras_machine_reward)]
    pub(super) type ErasMachineReward<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, EraIndex, Blake2_128Concat, MachineId, BalanceOf<T>, ValueQuery>;

    /// 某个Era机器释放的总奖励
    #[pallet::storage]
    #[pallet::getter(fn eras_machine_released_reward)]
    pub(super) type ErasMachineReleasedReward<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, EraIndex, Blake2_128Concat, MachineId, BalanceOf<T>, ValueQuery>;

    /// 某个Era Stash获得的总奖励
    #[pallet::storage]
    #[pallet::getter(fn eras_stash_reward)]
    pub(super) type ErasStashReward<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, EraIndex, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    /// 某个Era Stash解锁的总奖励
    #[pallet::storage]
    #[pallet::getter(fn eras_stash_released_reward)]
    pub(super) type ErasStashReleasedReward<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, EraIndex, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    /// 资金账户的质押总计
    #[pallet::storage]
    #[pallet::getter(fn stash_stake)]
    pub(super) type StashStake<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

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
        ValueQuery,
    >;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_runtime_upgrade() -> Weight {
            0
        }

        fn on_initialize(block_number: T::BlockNumber) -> Weight {
            // Era开始时，生成当前Era和下一个Era的快照
            // 每个Era(2880个块)执行一次
            if block_number.saturated_into::<u64>() % BLOCK_PER_ERA == 1 {
                let current_era: u32 = (block_number.saturated_into::<u64>() / BLOCK_PER_ERA) as u32;
                CurrentEra::<T>::put(current_era);

                let era_reward = Self::current_era_reward().unwrap_or_default();
                EraReward::<T>::insert(current_era, era_reward);

                if current_era == 0 {
                    ErasStashPoints::<T>::insert(0, EraStashPoints { ..Default::default() });
                    ErasStashPoints::<T>::insert(1, EraStashPoints { ..Default::default() });
                    let init_value: BTreeMap<MachineId, MachineGradeStatus> = BTreeMap::new();
                    ErasMachinePoints::<T>::insert(0, init_value.clone());
                    ErasMachinePoints::<T>::insert(1, init_value);
                } else {
                    // 用当前的Era快照初始化下一个Era的信息
                    let current_era_stash_snapshot = Self::eras_stash_points(current_era).unwrap_or_default();
                    ErasStashPoints::<T>::insert(current_era + 1, current_era_stash_snapshot);
                    let current_era_machine_snapshot = Self::eras_machine_points(current_era).unwrap_or_default();
                    ErasMachinePoints::<T>::insert(current_era + 1, current_era_machine_snapshot);
                }
            }
            Self::check_offline_machine_duration();
            0
        }

        fn on_finalize(block_number: T::BlockNumber) {
            let current_height = block_number.saturated_into::<u64>();

            // 在每个Era结束时执行奖励，发放到用户的Machine
            // 计算奖励，直接根据当前得分即可
            if current_height > 0 && current_height % BLOCK_PER_ERA == 0 {
                Self::distribute_reward();
            }

            let _ = Self::do_pending_slash();
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// When reward start to distribute
        #[pallet::weight(0)]
        pub fn set_reward_info(
            origin: OriginFor<T>,
            reward_info: PhaseRewardInfoDetail<BalanceOf<T>>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            <PhaseRewardInfo<T>>::put(reward_info);
            Ok(().into())
        }

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
        #[pallet::weight(0)]
        pub fn set_standard_gpu_point_price(
            origin: OriginFor<T>,
            point_price: StandardGpuPointPrice,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StandardGPUPointPrice::<T>::put(point_price);
            Ok(().into())
        }

        /// Stash account set a controller
        #[pallet::weight(10000)]
        pub fn set_controller(origin: OriginFor<T>, controller: T::AccountId) -> DispatchResultWithPostInfo {
            let stash = ensure_signed(origin)?;
            // Not allow multiple stash have same controller
            ensure!(!<ControllerStash<T>>::contains_key(&controller), Error::<T>::AlreadyController);

            StashController::<T>::insert(stash.clone(), controller.clone());
            ControllerStash::<T>::insert(controller.clone(), stash.clone());

            Self::deposit_event(Event::ControllerStashBonded(controller, stash));
            Ok(().into())
        }

        /// Stash account reset controller for one machine
        #[pallet::weight(10000)]
        pub fn stash_reset_controller(
            origin: OriginFor<T>,
            machine_id: MachineId,
            new_controller: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let stash = ensure_signed(origin)?;
            ensure!(!<ControllerStash<T>>::contains_key(&new_controller), Error::<T>::AlreadyController);

            let mut machine_info = Self::machines_info(&machine_id);
            let old_controller = machine_info.controller.clone();

            let mut old_controller_machines = Self::controller_machines(&old_controller);
            let mut new_controller_machines = Self::controller_machines(&new_controller);

            ensure!(machine_info.machine_stash == stash, Error::<T>::NotMachineStash);
            machine_info.controller = new_controller.clone();

            // Change controller_machines
            ItemList::rm_item(&mut old_controller_machines, &machine_id);
            ItemList::add_item(&mut new_controller_machines, machine_id.clone());

            ControllerMachines::<T>::insert(&old_controller, old_controller_machines);
            ControllerMachines::<T>::insert(&new_controller, new_controller_machines);
            MachinesInfo::<T>::insert(machine_id.clone(), machine_info);
            Self::deposit_event(Event::MachineControllerChanged(machine_id, old_controller, new_controller));
            Ok(().into())
        }

        /// Controller account reonline machine, allow change hardware info
        /// Committee will verify it later
        /// NOTE: User need to add machine basic info(pos & net speed), after
        /// committee verify finished, will be slashed for `OnlineReportOffline`
        #[pallet::weight(10000)]
        pub fn offline_machine_change_hardware_info(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut live_machines = Self::live_machines();
            let mut machine_info = Self::machines_info(&machine_id);

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);
            // 只允许在线状态的机器修改信息
            ensure!(machine_info.machine_status == MachineStatus::Online, Error::<T>::MachineStatusNotAllowed);

            // 重新上链需要质押一定的手续费
            let online_stake_params = Self::online_stake_params().ok_or(Error::<T>::GetReonlineStakeFailed)?;
            let stake_amount = T::DbcPrice::get_dbc_amount_by_value(online_stake_params.reonline_stake)
                .ok_or(Error::<T>::GetReonlineStakeFailed)?;

            machine_info.machine_status = MachineStatus::StakerReportOffline(now, Box::new(MachineStatus::Online));

            ItemList::rm_item(&mut live_machines.online_machine, &machine_id);
            ItemList::add_item(&mut live_machines.bonding_machine, machine_id.clone());

            Self::change_user_total_stake(machine_info.machine_stash.clone(), stake_amount, true)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;
            UserReonlineStake::<T>::insert(
                &machine_info.machine_stash,
                &machine_id,
                UserReonlineStakeInfo { stake_amount, offline_time: now },
            );
            Self::change_pos_gpu_by_online(&machine_id, false);
            Self::update_snap_by_online_status(machine_id.clone(), false);
            LiveMachines::<T>::put(live_machines);
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::MachineOfflineToMutHardware(machine_id, stake_amount));
            Ok(().into())
        }

        /// Controller account submit online request machine
        #[pallet::weight(10000)]
        pub fn bond_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            msg: Vec<u8>,
            sig: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashBond)?;
            let mut live_machines = Self::live_machines();
            let mut controller_machines = Self::controller_machines(&controller);
            let mut stash_machines = Self::stash_machines(&stash);

            ensure!(!live_machines.machine_id_exist(&machine_id), Error::<T>::MachineIdExist);
            // 验证msg: len(pubkey + account) = 64 + 48
            ensure!(msg.len() == 112, Error::<T>::BadMsgLen);

            let sig_machine_id: Vec<u8> = msg[..64].to_vec();
            ensure!(machine_id == sig_machine_id, Error::<T>::SigMachineIdNotEqualBondedMachineId);

            let sig_stash_account: Vec<u8> = msg[64..].to_vec();
            let sig_stash_account =
                Self::get_account_from_str(&sig_stash_account).ok_or(Error::<T>::ConvertMachineIdToWalletFailed)?;
            ensure!(sig_stash_account == stash, Error::<T>::MachineStashNotEqualControllerStash);

            // 验证签名是否为MachineId发出
            ensure!(Self::verify_sig(msg.clone(), sig.clone(), machine_id.clone()).is_some(), Error::<T>::BadSignature);

            // 用户绑定机器需要质押一张显卡的DBC
            let stake_amount = Self::stake_per_gpu().ok_or(Error::<T>::CalcStakeAmountFailed)?;

            // 扣除10个Dbc作为交易手续费
            <generic_func::Module<T>>::pay_fixed_tx_fee(controller.clone()).map_err(|_| Error::<T>::PayTxFeeFailed)?;

            ItemList::add_item(&mut stash_machines.total_machine, machine_id.clone());
            ItemList::add_item(&mut controller_machines, machine_id.clone());

            // 添加到LiveMachine的bonding_machine字段
            ItemList::add_item(&mut live_machines.bonding_machine, machine_id.clone());

            // 初始化MachineInfo, 并添加到MachinesInfo
            let machine_info = MachineInfo {
                controller: controller.clone(),
                machine_stash: stash.clone(),
                bonding_height: <frame_system::Module<T>>::block_number(),
                init_stake_per_gpu: stake_amount,
                stake_amount,
                machine_status: MachineStatus::AddingCustomizeInfo,
                ..Default::default()
            };

            Self::change_user_total_stake(stash.clone(), stake_amount, true)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;

            ControllerMachines::<T>::insert(&controller, controller_machines);
            StashMachines::<T>::insert(&stash, stash_machines);
            LiveMachines::<T>::put(live_machines);
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::BondMachine(controller.clone(), machine_id.clone(), stake_amount));
            Ok(().into())
        }

        /// Controller generate new server room id, record to stash account
        #[pallet::weight(10000)]
        pub fn gen_server_room(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashBond)?;

            <generic_func::Module<T>>::pay_fixed_tx_fee(controller.clone()).map_err(|_| Error::<T>::PayTxFeeFailed)?;

            let mut stash_server_rooms = Self::stash_server_rooms(&stash);
            let new_server_room = <generic_func::Module<T>>::random_server_room();
            ItemList::add_item(&mut stash_server_rooms, new_server_room);

            StashServerRooms::<T>::insert(&stash, stash_server_rooms);
            Self::deposit_event(Event::ServerRoomGenerated(controller.clone(), new_server_room));
            Ok(().into())
        }

        /// Controller add machine pos & net info
        #[pallet::weight(10000)]
        pub fn add_machine_info(
            origin: OriginFor<T>,
            machine_id: MachineId,
            customize_machine_info: StakerCustomizeInfo,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            ensure!(customize_machine_info.telecom_operators.len() > 0, Error::<T>::TelecomIsNull);
            // 查询机器Id是否在该账户的控制下
            let mut machine_info = Self::machines_info(&machine_id);
            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);

            let stash_server_rooms = Self::stash_server_rooms(&machine_info.machine_stash);
            ensure!(
                stash_server_rooms.binary_search(&customize_machine_info.server_room).is_ok(),
                Error::<T>::ServerRoomNotFound
            );

            match machine_info.machine_status {
                MachineStatus::AddingCustomizeInfo |
                MachineStatus::CommitteeVerifying |
                MachineStatus::CommitteeRefused(_) |
                MachineStatus::WaitingFulfill |
                MachineStatus::StakerReportOffline(_, _) => {
                    machine_info.machine_info_detail.staker_customize_info = customize_machine_info;
                },
                _ => return Err(Error::<T>::NotAllowedChangeMachineInfo.into()),
            }

            let mut live_machines = Self::live_machines();

            if live_machines.bonding_machine.binary_search(&machine_id).is_ok() {
                ItemList::rm_item(&mut live_machines.bonding_machine, &machine_id);
                ItemList::add_item(&mut live_machines.confirmed_machine, machine_id.clone());
                LiveMachines::<T>::put(live_machines);
                machine_info.machine_status = MachineStatus::DistributingOrder;
            }

            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::MachineInfoAdded(machine_id));
            Ok(().into())
        }

        /// 机器第一次上线后处于补交质押状态时，需要补交质押才能上线
        #[pallet::weight(10000)]
        pub fn fulfill_machine(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let current_era = Self::current_era();

            let mut machine_info = Self::machines_info(&machine_id);
            let mut live_machine = Self::live_machines();

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);
            ensure!(machine_info.online_height == Zero::zero(), Error::<T>::MachineStatusNotAllowed);

            // NOTE: 机器补交质押时，所需的质押 = max(当前机器需要的质押，第一次绑定上线时的质押量)
            // 每卡质押按照第一次上线时计算
            let stake_need = machine_info
                .init_stake_per_gpu
                .checked_mul(
                    &machine_info.machine_info_detail.committee_upload_info.gpu_num.saturated_into::<BalanceOf<T>>(),
                )
                .ok_or(Error::<T>::CalcStakeAmountFailed)?;

            // 当出现需要补交质押时
            if machine_info.stake_amount < stake_need {
                let extra_stake = stake_need - machine_info.stake_amount;
                Self::change_user_total_stake(machine_info.machine_stash.clone(), extra_stake, true)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;
                machine_info.stake_amount = stake_need;
            }
            machine_info.machine_status = MachineStatus::Online;

            if UserReonlineStake::<T>::contains_key(&machine_info.machine_stash, &machine_id) {
                // 根据质押，奖励给这些委员会
                let reonline_stake = Self::user_reonline_stake(&machine_info.machine_stash, &machine_id);

                // 根据下线时间，惩罚stash
                let offline_duration = now - reonline_stake.offline_time;
                // 如果下线的时候空闲超过10天，则不进行惩罚
                if reonline_stake.offline_time < machine_info.last_online_height + 28800u32.into() {
                    Self::slash_when_report_offline(
                        machine_id.clone(),
                        OPSlashReason::OnlineReportOffline(offline_duration),
                        None,
                        None,
                    );
                }
                // 退还reonline_stake
                Self::change_user_total_stake(machine_info.machine_stash.clone(), reonline_stake.stake_amount, false)
                    .map_err(|_| Error::<T>::ReduceStakeFailed)?;
                UserReonlineStake::<T>::remove(&machine_info.machine_stash, &machine_id);
            } else {
                machine_info.online_height = now;
                machine_info.reward_deadline = current_era + REWARD_DURATION;
            }

            machine_info.last_online_height = now;
            machine_info.last_machine_restake = now;

            Self::change_pos_gpu_by_online(&machine_id, true);
            Self::update_snap_by_online_status(machine_id.clone(), true);

            ItemList::rm_item(&mut live_machine.fulfilling_machine, &machine_id);
            ItemList::add_item(&mut live_machine.online_machine, machine_id.clone());

            LiveMachines::<T>::put(live_machine);
            MachinesInfo::<T>::insert(&machine_id, machine_info);
            Ok(().into())
        }

        /// 控制账户进行领取收益到stash账户
        #[pallet::weight(10000)]
        pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash_account = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashAccount)?;
            ensure!(StashMachines::<T>::contains_key(&stash_account), Error::<T>::NotMachineController);
            let mut stash_machine = Self::stash_machines(&stash_account);
            let can_claim = stash_machine.can_claim_reward;

            stash_machine.total_claimed_reward =
                stash_machine.total_claimed_reward.checked_add(&can_claim).ok_or(Error::<T>::ClaimRewardFailed)?;
            stash_machine.can_claim_reward = Zero::zero();

            <T as pallet::Config>::Currency::deposit_into_existing(&stash_account, can_claim)
                .map_err(|_| Error::<T>::ClaimRewardFailed)?;

            StashMachines::<T>::insert(&stash_account, stash_machine);
            Self::deposit_event(Event::ClaimReward(stash_account, can_claim));
            Ok(().into())
        }

        /// 控制账户报告机器下线:Online/Rented时允许
        #[pallet::weight(10000)]
        pub fn controller_report_offline(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let machine_info = Self::machines_info(&machine_id);

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);

            // 某些状态允许下线
            match machine_info.machine_status {
                MachineStatus::Online | MachineStatus::Rented => {},
                _ => return Err(Error::<T>::MachineStatusNotAllowed.into()),
            }

            Self::machine_offline(
                machine_id.clone(),
                MachineStatus::StakerReportOffline(now, Box::new(machine_info.machine_status)),
            );

            Self::deposit_event(Event::ControllerReportOffline(machine_id));
            Ok(().into())
        }

        /// 控制账户报告机器上线
        #[pallet::weight(10000)]
        pub fn controller_report_online(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let current_era = Self::current_era();

            let mut machine_info = Self::machines_info(&machine_id);
            let mut sys_info = Self::sys_info();
            let mut stash_machine = Self::stash_machines(&machine_info.machine_stash);
            let mut live_machine = Self::live_machines();

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);
            ensure!(
                now - machine_info.last_online_height > (BLOCK_PER_ERA as u32).saturated_into::<T::BlockNumber>(),
                Error::<T>::CannotOnlineTwiceOneDay
            );

            let mut slash_amount = Zero::zero();
            // MachineStatus改为之前的状态
            match machine_info.machine_status.clone() {
                MachineStatus::StakerReportOffline(offline_time, status) => {
                    let offline_duration = now - offline_time;

                    machine_info.machine_status = *status;
                    let gpu_num = machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;

                    match machine_info.machine_status {
                        MachineStatus::Online | MachineStatus::Rented => {
                            // Both status will change grades by online
                            sys_info.total_gpu_num += gpu_num;

                            let old_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);

                            Self::update_snap_by_online_status(machine_id.clone(), true);
                            Self::change_pos_gpu_by_online(&machine_id, true);

                            let new_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);
                            stash_machine.total_calc_points =
                                stash_machine.total_calc_points + new_stash_grade - old_stash_grade;
                            sys_info.total_calc_points = sys_info.total_calc_points + new_stash_grade - old_stash_grade;

                            ItemList::add_item(&mut stash_machine.online_machine, machine_id.clone());

                            stash_machine.total_gpu_num += gpu_num;
                        },
                        _ => {},
                    }
                    match machine_info.machine_status {
                        MachineStatus::Online => {
                            // 如果在线超过10天，则不进行惩罚超过
                            if offline_time < machine_info.last_online_height + 28800u32.into() {
                                // 机器在被租用状态下线，会被惩罚
                                slash_amount = Self::slash_when_report_offline(
                                    machine_id.clone(),
                                    OPSlashReason::RentedReportOffline(offline_duration),
                                    None,
                                    None,
                                );
                            }
                        },
                        MachineStatus::Rented => {
                            Self::update_snap_by_rent_status(machine_id.clone(), true);
                            Self::change_pos_gpu_by_rent(&machine_id, true);

                            // 机器在被租用状态下线，会被惩罚
                            slash_amount = Self::slash_when_report_offline(
                                machine_id.clone(),
                                OPSlashReason::RentedReportOffline(offline_duration),
                                None,
                                None,
                            );
                        },
                        _ => {},
                    }
                },
                MachineStatus::ReporterReportOffline(slash_reason, _status, reporter, committee) => {
                    slash_amount = Self::slash_when_report_offline(
                        machine_id.clone(),
                        slash_reason,
                        Some(reporter),
                        Some(committee),
                    );
                },
                _ => return Err(Error::<T>::MachineStatusNotAllowed.into()),
            }

            SysInfo::<T>::put(sys_info); // Must before `change_user_total_stake`
            if slash_amount != Zero::zero() {
                Self::change_user_total_stake(machine_info.machine_stash.clone(), slash_amount, true)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;
            }

            machine_info.last_online_height = now;

            ItemList::rm_item(&mut live_machine.offline_machine, &machine_id);
            match machine_info.machine_status {
                MachineStatus::WaitingFulfill => {
                    ItemList::add_item(&mut live_machine.fulfilling_machine, machine_id.clone());
                },
                _ => {
                    ItemList::add_item(&mut live_machine.online_machine, machine_id.clone());
                },
            }

            LiveMachines::<T>::put(live_machine);
            StashMachines::<T>::insert(&machine_info.machine_stash, stash_machine);
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::ControllerReportOnline(machine_id));
            Ok(().into())
        }

        /// 超过365天的机器可以在距离上次租用10天，且没被租用时退出
        #[pallet::weight(10000)]
        pub fn machine_exit(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let mut machine_info = Self::machines_info(&machine_id);
            let now = <frame_system::Module<T>>::block_number();
            let current_era = Self::current_era();

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);
            ensure!(MachineStatus::Online == machine_info.machine_status, Error::<T>::MachineStatusNotAllowed);
            // 确保机器：奖励结束时间 - 1年即为上线时间
            ensure!(machine_info.reward_deadline <= current_era + 365, Error::<T>::TimeNotAllowed);
            // 确保机器距离上次租用超过10天
            ensure!(now - machine_info.last_online_height >= 28800u32.into(), Error::<T>::TimeNotAllowed);

            // 下线机器，并退还奖励
            Self::change_pos_gpu_by_online(&machine_id, false);
            Self::update_snap_by_online_status(machine_id.clone(), false);
            Self::change_user_total_stake(machine_info.machine_stash.clone(), machine_info.stake_amount, false)
                .map_err(|_| Error::<T>::ReduceStakeFailed)?;
            machine_info.stake_amount = Zero::zero();
            machine_info.machine_status = MachineStatus::Exit;

            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::MachineExit(machine_id));
            Ok(().into())
        }

        /// 满足365天可以申请重新质押，退回质押币
        /// 在系统中上线满365天之后，可以按当时机器需要的质押数量，重新入网。多余的币解绑
        /// 在重新上线之后，下次再执行本操作，需要等待365天
        #[pallet::weight(10000)]
        pub fn restake_online_machine(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let mut machine_info = Self::machines_info(&machine_id);
            let old_stake = machine_info.stake_amount;

            ensure!(controller == machine_info.controller, Error::<T>::NotMachineController);
            ensure!(now - machine_info.last_machine_restake >= REBOND_FREQUENCY.into(), Error::<T>::TooFastToReStake);

            let stake_per_gpu = Self::stake_per_gpu().ok_or(Error::<T>::CalcStakeAmountFailed)?;
            let stake_need = stake_per_gpu
                .checked_mul(
                    &machine_info.machine_info_detail.committee_upload_info.gpu_num.saturated_into::<BalanceOf<T>>(),
                )
                .ok_or(Error::<T>::CalcStakeAmountFailed)?;
            ensure!(machine_info.stake_amount > stake_need, Error::<T>::NoStakeToReduce);

            let extra_stake =
                machine_info.stake_amount.checked_sub(&stake_need).ok_or(Error::<T>::ReduceStakeFailed)?;

            machine_info.stake_amount = stake_need;
            machine_info.last_machine_restake = now;
            machine_info.init_stake_per_gpu = stake_per_gpu;
            Self::change_user_total_stake(machine_info.machine_stash.clone(), extra_stake, false)
                .map_err(|_| Error::<T>::ReduceStakeFailed)?;

            MachinesInfo::<T>::insert(&machine_id, machine_info.clone());
            Self::deposit_event(Event::MachineRestaked(machine_id, old_stake, machine_info.stake_amount));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn apply_slash_review(origin: OriginFor<T>, slash_id: u64) -> DispatchResultWithPostInfo {
            // TODO
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn cancel_slash(origin: OriginFor<T>, slash_id: u64) -> DispatchResultWithPostInfo {
            T::CancelSlashOrigin::ensure_origin(origin)?;
            ensure!(PendingSlash::<T>::contains_key(slash_id), Error::<T>::SlashIdNotExist);

            let slash_info = Self::pending_slash(slash_id);

            Self::change_user_total_stake(slash_info.slash_who.clone(), slash_info.slash_amount, false)
                .map_err(|_| Error::<T>::ReduceStakeFailed)?;
            PendingSlash::<T>::remove(slash_id);

            Self::deposit_event(Event::SlashCanceled(slash_id, slash_info.slash_who, slash_info.slash_amount));
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        BondMachine(T::AccountId, MachineId, BalanceOf<T>),
        Slash(T::AccountId, BalanceOf<T>, OPSlashReason<T::BlockNumber>),
        ControllerStashBonded(T::AccountId, T::AccountId),
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
        CannotOnlineTwiceOneDay,
        ServerRoomNotFound,
        NotMachineStash,
        TooFastToReStake,
        NoStakeToReduce,
        ReduceStakeFailed,
        GetReonlineStakeFailed,
        SlashIdNotExist,
        TimeNotAllowed,
    }
}

impl<T: Config> Pallet<T> {
    // 主动惩罚超过下线阈值的机器
    fn check_offline_machine_duration() {
        let live_machine = Self::live_machines();
        let now = <frame_system::Module<T>>::block_number();

        for a_machine in live_machine.offline_machine {
            let machine_info = Self::machines_info(&a_machine);
            match machine_info.machine_status {
                MachineStatus::StakerReportOffline(offline_time, status) => {
                    if now - offline_time < MAX_SLASH_THRESHOLD.into() {
                        continue
                    }

                    match *status {
                        MachineStatus::Online => {
                            Self::add_offline_slash(
                                50,
                                a_machine,
                                machine_info.last_machine_renter,
                                None,
                                OPSlashReason::OnlineReportOffline(offline_time),
                            );
                        },

                        MachineStatus::Rented => {
                            if now - offline_time < (2 * MAX_SLASH_THRESHOLD).into() {
                                continue
                            }
                            Self::add_offline_slash(
                                80,
                                a_machine,
                                None,
                                None,
                                OPSlashReason::RentedReportOffline(offline_time),
                            );
                        },
                        _ => continue,
                    }
                },
                MachineStatus::ReporterReportOffline(offline_reason, _status, _reporter, committee) => {
                    match offline_reason {
                        // 被举报时
                        OPSlashReason::RentedInaccessible(report_time) |
                        OPSlashReason::RentedHardwareCounterfeit(report_time) |
                        OPSlashReason::RentedHardwareMalfunction(report_time) |
                        OPSlashReason::OnlineRentFailed(report_time) => {
                            if now - report_time < MAX_SLASH_THRESHOLD.into() {
                                continue
                            }
                            Self::add_offline_slash(
                                100,
                                a_machine,
                                machine_info.last_machine_renter,
                                Some(committee),
                                offline_reason,
                            );
                        },

                        _ => continue,
                    }
                },
                _ => continue,
            }
        }
    }

    // Return slashed amount when slash is executed
    fn slash_when_report_offline(
        machine_id: MachineId,
        slash_reason: OPSlashReason<T::BlockNumber>,
        reporter: Option<T::AccountId>,
        committee: Option<Vec<T::AccountId>>,
    ) -> BalanceOf<T> {
        let machine_info = Self::machines_info(&machine_id);
        let now = <frame_system::Module<T>>::block_number();
        match slash_reason {
            // 算工主动报告被租用的机器，主动下线
            OPSlashReason::RentedReportOffline(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return Zero::zero(),
                    // 下线不超过7分钟
                    1..=14 => {
                        // 扣除2%质押币。100%进入国库。
                        return Self::add_offline_slash(2, machine_id, None, None, slash_reason)
                    },
                    // 不超过48小时
                    15..=5760 => {
                        // 扣除4%质押币。100%进入国库
                        return Self::add_offline_slash(4, machine_id, None, None, slash_reason)
                    },
                    // 不超过120小时
                    5761..=14400 => {
                        // 扣除30%质押币，10%给到用户，90%进入国库
                        return Self::add_offline_slash(
                            30,
                            machine_id,
                            machine_info.last_machine_renter,
                            None,
                            slash_reason,
                        )
                    },
                    // 超过120小时
                    _ => {
                        // 扣除50%押金。10%给到用户，90%进入国库
                        return Self::add_offline_slash(
                            50,
                            machine_id,
                            machine_info.last_machine_renter,
                            None,
                            slash_reason,
                        )
                    },
                }
            },
            // 算工主动报告在线的机器，主动下线
            OPSlashReason::OnlineReportOffline(duration) => {
                // 判断是否已经下线十天，如果是，则不进行惩罚，仅仅下线处理
                // NOTE: 此时，machine_info.last_online_height还未改变
                if now > 28800u32.saturated_into::<T::BlockNumber>() + duration + machine_info.last_online_height {
                    return Zero::zero()
                }
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return Zero::zero(),
                    // 下线不超过7分钟
                    1..=14 => {
                        // 扣除2%质押币，质押币全部进入国库。
                        return Self::add_offline_slash(2, machine_id, None, None, slash_reason)
                    },
                    // 下线不超过48小时
                    15..=5760 => {
                        // 扣除4%质押币，质押币全部进入国库
                        return Self::add_offline_slash(4, machine_id, None, None, slash_reason)
                    },
                    // 不超过240小时
                    5761..=28800 => {
                        // 扣除30%质押币，质押币全部进入国库
                        return Self::add_offline_slash(30, machine_id, None, None, slash_reason)
                    },
                    _ => {
                        // TODO: 如果机器从首次上线时间起超过365天，剩下20%押金可以申请退回。

                        // 扣除80%质押币。质押币全部进入国库。
                        return Self::add_offline_slash(80, machine_id, None, None, slash_reason)
                    },
                }
            },
            // 机器处于租用状态，无法访问，这种情况下，reporter == renter
            OPSlashReason::RentedInaccessible(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return Zero::zero(),
                    // 不超过7分钟
                    1..=14 => {
                        // 扣除4%质押币。10%给验证人，90%进入国库
                        return Self::add_offline_slash(4, machine_id, None, committee, slash_reason)
                    },
                    // 不超过48小时
                    15..=5760 => {
                        // 扣除8%质押币。10%给验证人，90%进入国库
                        return Self::add_offline_slash(8, machine_id, None, committee, slash_reason)
                    },
                    // 不超过120小时
                    5761..=14400 => {
                        // 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(60, machine_id, reporter, committee, slash_reason)
                    },
                    // 超过120小时
                    _ => {
                        // 扣除100%押金。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(100, machine_id, reporter, committee, slash_reason)
                    },
                }
            },
            // 机器处于租用状态，机器出现故障
            OPSlashReason::RentedHardwareMalfunction(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return Zero::zero(),
                    //不超过4小时
                    1..=480 => {
                        // 扣除6%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(6, machine_id, reporter, committee, slash_reason)
                    },
                    // 不超过24小时
                    481..=2880 => {
                        // 扣除12%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(12, machine_id, reporter, committee, slash_reason)
                    },
                    // 不超过48小时
                    2881..=5760 => {
                        // 扣除16%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(16, machine_id, reporter, committee, slash_reason)
                    },
                    // 不超过120小时
                    5761..=14400 => {
                        // 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(60, machine_id, reporter, committee, slash_reason)
                    },
                    _ => {
                        // 扣除100%押金，10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(100, machine_id, reporter, committee, slash_reason)
                    },
                }
            },
            // 机器处于租用状态，机器硬件造假
            OPSlashReason::RentedHardwareCounterfeit(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return Zero::zero(),
                    // 下线不超过4小时
                    1..=480 => {
                        // 扣除12%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(12, machine_id, reporter, committee, slash_reason)
                    },
                    // 不超过24小时
                    481..=2880 => {
                        // 扣除24%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(24, machine_id, reporter, committee, slash_reason)
                    },
                    // 不超过48小时
                    2881..=5760 => {
                        // 扣除32%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(32, machine_id, reporter, committee, slash_reason)
                    },
                    // 不超过120小时
                    5761..=14400 => {
                        // 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(60, machine_id, reporter, committee, slash_reason)
                    },
                    _ => {
                        // 扣除100%押金，10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(100, machine_id, reporter, committee, slash_reason)
                    },
                }
            },
            // 机器在线，被举报无法租用
            OPSlashReason::OnlineRentFailed(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return Zero::zero(),
                    1..=480 => {
                        // 扣除6%质押币。10%给到用户，20%给到验证人，50%进入国库
                        return Self::add_offline_slash(6, machine_id, reporter, committee, slash_reason)
                    },
                    481..=2880 => {
                        // 扣除12%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(12, machine_id, reporter, committee, slash_reason)
                    },
                    2881..=5760 => {
                        // 扣除16%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(16, machine_id, reporter, committee, slash_reason)
                    },
                    5761..=14400 => {
                        // 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(60, machine_id, reporter, committee, slash_reason)
                    },
                    _ => {
                        // 扣除100%押金，10%给到用户，20%给到验证人，70%进入国库
                        return Self::add_offline_slash(100, machine_id, reporter, committee, slash_reason)
                    },
                }
            },
            _ => return Zero::zero(),
        }
    }

    fn add_offline_slash(
        slash_percent: u32,
        machine_id: MachineId,
        reporter: Option<T::AccountId>,
        committee: Option<Vec<T::AccountId>>,
        slash_reason: OPSlashReason<T::BlockNumber>,
    ) -> BalanceOf<T> {
        let now = <frame_system::Module<T>>::block_number();
        let machine_info = Self::machines_info(&machine_id);
        let slash_amount = Perbill::from_rational_approximation(slash_percent, 100) * machine_info.stake_amount;

        let slash_id = Self::get_new_slash_id();
        let slash_info = OPPendingSlashInfo {
            slash_who: machine_info.machine_stash,
            machine_id,
            slash_time: now,
            slash_amount,
            slash_exec_time: now + (2880u32 * 2).saturated_into::<T::BlockNumber>(),
            reward_to_reporter: reporter,
            reward_to_committee: committee,
            slash_reason,
        };

        PendingSlash::<T>::insert(slash_id, slash_info);
        slash_amount
    }

    // FIXME: refa, and should not do transfer but unreserve
    // 惩罚掉机器押金，如果执行惩罚后机器押金不够，则状态变为补充质押
    fn do_slash_deposit(slash_info: &OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>) {
        let machine_info = Self::machines_info(&slash_info.machine_id);

        // FIXME: 检查分配比例;执行惩罚
        // 根据比例，分配slash_amoun
        let (slash_to_treasury, reward_to_reporter, reward_to_committee) = {
            let percent_10 = Perbill::from_rational_approximation(10u32, 100u32);
            let percent_20 = Perbill::from_rational_approximation(20u32, 100u32);

            if slash_info.reward_to_reporter.is_some() && slash_info.reward_to_committee.is_none() {
                let reward_to_reporter = percent_10 * slash_info.slash_amount;
                let slash_to_treasury = slash_info.slash_amount - reward_to_reporter;
                (slash_to_treasury, reward_to_reporter, Zero::zero())
            } else if slash_info.reward_to_reporter.is_some() && slash_info.reward_to_committee.is_some() {
                let reward_to_reporter = percent_10 * slash_info.slash_amount;
                let reward_to_committee = percent_20 * slash_info.slash_amount;
                let slash_to_treasury = slash_info.slash_amount - reward_to_reporter - reward_to_committee;
                (slash_to_treasury, reward_to_reporter, reward_to_committee)
            } else {
                (slash_info.slash_amount, Zero::zero(), Zero::zero())
            }
        };

        // 奖励给委员会的立即给委员会
        if !reward_to_reporter.is_zero() && slash_info.reward_to_reporter.is_some() {
            let _ = <T as pallet::Config>::Currency::transfer(
                &machine_info.machine_stash,
                &slash_info.reward_to_reporter.as_ref().unwrap(),
                reward_to_reporter,
                KeepAlive,
            );
        }

        // 奖励给报告人的立即给报告人
        if !reward_to_committee.is_zero() && slash_info.reward_to_committee.is_some() {
            let committees = slash_info.reward_to_committee.as_ref().unwrap();
            let reward_each_committee_get =
                Perbill::from_rational_approximation(1u32, committees.len() as u32) * reward_to_committee;
            for a_committee in committees {
                let _ = <T as pallet::Config>::Currency::transfer(
                    &machine_info.machine_stash,
                    &a_committee,
                    reward_each_committee_get,
                    KeepAlive,
                );
            }
        }

        // 执行惩罚
        if <T as pallet::Config>::Currency::reserved_balance(&machine_info.machine_stash) >= slash_to_treasury {
            let (imbalance, _missing) =
                <T as pallet::Config>::Currency::slash_reserved(&machine_info.machine_stash, slash_to_treasury);
            // Self::deposit_event(Event::Slash(machine_info.machine_stash.clone(), slash_amount, SlashReason::));
            <T as pallet::Config>::Slash::on_unbalanced(imbalance);
        }
    }

    fn get_new_slash_id() -> u64 {
        let slash_id = Self::next_slash_id();

        if slash_id == u64::MAX {
            NextSlashId::<T>::put(0);
        } else {
            NextSlashId::<T>::put(slash_id + 1);
        };

        slash_id
    }

    fn do_pending_slash() -> Result<(), ()> {
        // 获得所有slashID
        let now = <frame_system::Module<T>>::block_number();
        let all_slash_id = <PendingSlash<T> as IterableStorageMap<u64, _>>::iter()
            .map(|(slash_id, _)| slash_id)
            .collect::<Vec<_>>();

        // 判断是否已经超过2天，如果超过，则执行惩罚
        for slash_id in all_slash_id {
            let slash_info = Self::pending_slash(slash_id);
            if now < slash_info.slash_exec_time {
                continue
            }

            match slash_info.slash_reason {
                OPSlashReason::CommitteeRefusedOnline | OPSlashReason::CommitteeRefusedMutHardware => {
                    Self::slash_and_reward(
                        slash_info.slash_who,
                        slash_info.slash_amount,
                        slash_info.slash_reason,
                        slash_info.reward_to_committee.unwrap_or_default(),
                    )?;
                },
                _ => {
                    Self::do_slash_deposit(&slash_info);
                    continue
                },
            }

            PendingSlash::<T>::remove(slash_id);
        }

        Ok(())
    }

    // For upgrade
    pub fn get_all_machine_id() -> Vec<MachineId> {
        <MachinesInfo<T> as IterableStorageMap<MachineId, _>>::iter()
            .map(|(machine_id, _)| machine_id)
            .collect::<Vec<_>>()
    }

    /// 下架机器
    fn machine_offline(machine_id: MachineId, machine_status: MachineStatus<T::BlockNumber, T::AccountId>) {
        let mut machine_info = Self::machines_info(&machine_id);
        let mut live_machine = Self::live_machines();

        if let MachineStatus::Rented = machine_info.machine_status {
            Self::update_snap_by_rent_status(machine_id.clone(), false);
            Self::change_pos_gpu_by_rent(&machine_id, false);
        }

        // When offline, pos_info will be removed
        Self::change_pos_gpu_by_online(&machine_id, false);
        Self::update_snap_by_online_status(machine_id.clone(), false);

        ItemList::rm_item(&mut live_machine.online_machine, &machine_id);
        ItemList::add_item(&mut live_machine.offline_machine, machine_id.clone());

        // After re-online, machine status is same as former
        machine_info.machine_status = machine_status;

        LiveMachines::<T>::put(live_machine);
        MachinesInfo::<T>::insert(&machine_id, machine_info);
    }

    /// 特定位置GPU上线/下线
    // - Writes:
    // PosGPUInfo, ServerRoomMachines
    fn change_pos_gpu_by_online(machine_id: &MachineId, is_online: bool) {
        let machine_info = Self::machines_info(&machine_id);

        let longitude = machine_info.machine_info_detail.staker_customize_info.longitude.clone();
        let latitude = machine_info.machine_info_detail.staker_customize_info.latitude.clone();
        let gpu_num = machine_info.machine_info_detail.committee_upload_info.gpu_num;
        let calc_point = machine_info.machine_info_detail.committee_upload_info.calc_point;

        let mut pos_gpu_info = Self::pos_gpu_info(longitude.clone(), latitude.clone());

        if is_online {
            pos_gpu_info.online_gpu += gpu_num as u64;
            pos_gpu_info.online_gpu_calc_points += calc_point;
        } else {
            pos_gpu_info.online_gpu -= gpu_num as u64;
            pos_gpu_info.offline_gpu += gpu_num as u64;
            pos_gpu_info.online_gpu_calc_points -= calc_point;
        }

        let server_room = machine_info.machine_info_detail.staker_customize_info.server_room;
        let mut server_room_machines = Self::server_room_machines(server_room).unwrap_or_default();

        if is_online {
            ItemList::add_item(&mut server_room_machines, machine_id.to_vec());
        } else {
            ItemList::rm_item(&mut server_room_machines, machine_id);
        }

        ServerRoomMachines::<T>::insert(server_room, server_room_machines);
        PosGPUInfo::<T>::insert(longitude, latitude, pos_gpu_info);
    }

    /// 特定位置GPU被租用/租用结束
    fn change_pos_gpu_by_rent(machine_id: &MachineId, is_rented: bool) {
        let machine_info = Self::machines_info(machine_id);

        let longitude = machine_info.machine_info_detail.staker_customize_info.longitude.clone();
        let latitude = machine_info.machine_info_detail.staker_customize_info.latitude.clone();
        let gpu_num = machine_info.machine_info_detail.committee_upload_info.gpu_num;

        let mut pos_gpu_info = Self::pos_gpu_info(longitude.clone(), latitude.clone());
        if is_rented {
            pos_gpu_info.rented_gpu += gpu_num as u64;
        } else {
            pos_gpu_info.rented_gpu -= gpu_num as u64;
        }

        PosGPUInfo::<T>::insert(longitude, latitude, pos_gpu_info);
    }

    // 接收到[u8; 64] -> str -> [u8; 32] -> pubkey
    fn verify_sig(msg: Vec<u8>, sig: Vec<u8>, account: Vec<u8>) -> Option<()> {
        let signature = sp_core::sr25519::Signature::try_from(&sig[..]).ok()?;
        // let public = Self::get_public_from_str(&account)?;

        let pubkey_str = str::from_utf8(&account).ok()?;
        let pubkey_hex: Result<Vec<u8>, _> =
            (0..pubkey_str.len()).step_by(2).map(|i| u8::from_str_radix(&pubkey_str[i..i + 2], 16)).collect();
        let pubkey_hex = pubkey_hex.ok()?;

        let account_id32: [u8; 32] = pubkey_hex.try_into().ok()?;
        let public = sp_core::sr25519::Public::from_slice(&account_id32);

        signature.verify(&msg[..], &public.into()).then(|| ())
    }

    // 参考：primitives/core/src/crypto.rs: impl Ss58Codec for AccountId32
    // from_ss58check_with_version
    fn get_accountid32(addr: &Vec<u8>) -> Option<[u8; 32]> {
        let mut data: [u8; 35] = [0; 35];

        let length = bs58::decode(addr).into(&mut data).ok()?;
        if length != 35 {
            return None
        }

        let (_prefix_len, _ident) = match data[0] {
            0..=63 => (1, data[0] as u16),
            _ => return None,
        };

        let account_id32: [u8; 32] = data[1..33].try_into().ok()?;
        Some(account_id32)
    }

    fn get_account_from_str(addr: &Vec<u8>) -> Option<T::AccountId> {
        let account_id32: [u8; 32] = Self::get_accountid32(addr)?;
        T::AccountId::decode(&mut &account_id32[..]).ok()
    }

    fn _get_public_from_str(addr: &Vec<u8>) -> Option<sp_core::sr25519::Public> {
        let account_id32: [u8; 32] = Self::get_accountid32(addr)?;
        Some(sp_core::sr25519::Public::from_slice(&account_id32))
    }

    // 质押DBC机制：[0, 10000] GPU: 100000 DBC per GPU
    // (10000, +) -> min( 100000 * 10000 / (10000 + n), 5w RMB DBC
    pub fn stake_per_gpu() -> Option<BalanceOf<T>> {
        let sys_info = Self::sys_info();
        let online_stake_params = Self::online_stake_params()?;

        let dbc_stake_per_gpu = if sys_info.total_gpu_num > 10_000 {
            Perbill::from_rational_approximation(10_000u64, sys_info.total_gpu_num) *
                online_stake_params.online_stake_per_gpu
        } else {
            online_stake_params.online_stake_per_gpu
        };

        let stake_limit = T::DbcPrice::get_dbc_amount_by_value(online_stake_params.online_stake_usd_limit)?;
        Some(dbc_stake_per_gpu.min(stake_limit)) // .checked_mul(&gpu_num.saturated_into::<BalanceOf<T>>())
    }

    /// 根据GPU数量和该机器算力点数，计算该机器相比标准配置的租用价格
    pub fn calc_machine_price(machine_point: u64) -> Option<u64> {
        let standard_gpu_point_price = Self::standard_gpu_point_price()?;
        standard_gpu_point_price
            .gpu_price
            .checked_mul(machine_point)?
            .checked_mul(10_000)?
            .checked_div(standard_gpu_point_price.gpu_point)?
            .checked_div(10_000)
    }

    /// 计算当前Era在线奖励数量
    fn current_era_reward() -> Option<BalanceOf<T>> {
        let current_era = Self::current_era() as u64;
        let phase_reward_info = Self::phase_reward_info()?;

        let reward_start_era = phase_reward_info.online_reward_start_era as u64;
        let era_duration = (current_era >= reward_start_era).then(|| current_era - reward_start_era)?;

        let era_reward = if era_duration < phase_reward_info.first_phase_duration as u64 {
            phase_reward_info.phase_0_reward_per_era
        } else if era_duration < phase_reward_info.first_phase_duration as u64 + 1825 {
            // 365 * 5
            phase_reward_info.phase_1_reward_per_era
        } else {
            phase_reward_info.phase_2_reward_per_era
        };

        if Self::galaxy_is_on() && current_era < phase_reward_info.galaxy_on_era as u64 + 60 {
            Some(era_reward.checked_mul(&2u32.saturated_into::<BalanceOf<T>>())?)
        } else {
            Some(era_reward)
        }
    }

    fn slash_and_reward(
        slash_who: T::AccountId,
        slash_amount: BalanceOf<T>,
        slash_reason: OPSlashReason<T::BlockNumber>,
        reward_to: Vec<T::AccountId>,
    ) -> Result<(), ()> {
        let mut stash_stake = Self::stash_stake(&slash_who);
        let mut sys_info = Self::sys_info();

        sys_info.total_stake = sys_info.total_stake.checked_sub(&slash_amount).ok_or(())?;
        stash_stake = stash_stake.checked_sub(&slash_amount).ok_or(())?;

        let _ = T::SlashAndReward::slash_and_reward(vec![slash_who.clone()], slash_amount, reward_to);

        StashStake::<T>::insert(&slash_who, stash_stake);
        SysInfo::<T>::put(sys_info);
        Ok(())
    }

    fn change_user_total_stake(who: T::AccountId, amount: BalanceOf<T>, is_add: bool) -> Result<(), ()> {
        let mut stash_stake = Self::stash_stake(&who);
        let mut sys_info = Self::sys_info();

        if is_add {
            sys_info.total_stake = sys_info.total_stake.checked_add(&amount).ok_or(())?;
            stash_stake = stash_stake.checked_add(&amount).ok_or(())?;

            ensure!(<T as Config>::Currency::can_reserve(&who, amount), ());
            <T as pallet::Config>::Currency::reserve(&who, amount).map_err(|_| ())?;
        } else {
            stash_stake = stash_stake.checked_sub(&amount).ok_or(())?;
            sys_info.total_stake = sys_info.total_stake.checked_sub(&amount).ok_or(())?;
            <T as pallet::Config>::Currency::unreserve(&who, amount);
        }

        StashStake::<T>::insert(&who, stash_stake);
        SysInfo::<T>::put(sys_info);

        if is_add {
            Self::deposit_event(Event::StakeAdded(who, amount));
        } else {
            Self::deposit_event(Event::StakeReduced(who, amount));
        }
        Ok(())
    }

    fn reward_reonline_committee(
        who: T::AccountId,
        amount: BalanceOf<T>,
        committee: Vec<T::AccountId>,
    ) -> Result<(), ()> {
        Self::slash_and_reward(who, amount, OPSlashReason::ReonlineShouldReward, committee)
    }

    // 获取下一Era stash grade即为当前Era stash grade
    fn get_stash_grades(era_index: EraIndex, stash: &T::AccountId) -> u64 {
        let next_era_stash_snapshot = Self::eras_stash_points(era_index).unwrap_or_default();

        if let Some(stash_snapshot) = next_era_stash_snapshot.staker_statistic.get(stash) {
            return stash_snapshot.total_grades().unwrap_or_default()
        }
        0
    }

    // When Online:
    // - Writes:(currentEra) ErasStashPoints, ErasMachinePoints,
    //   SysInfo, StashMachines
    // When Offline:
    // - Writes: (currentEra) ErasStashPoints, ErasMachinePoints, (nextEra) ErasStashPoints, ErasMachinePoints
    //   SysInfo, StashMachines
    fn update_snap_by_online_status(machine_id: MachineId, is_online: bool) {
        let machine_info = Self::machines_info(&machine_id);
        let machine_base_info = machine_info.machine_info_detail.committee_upload_info.clone();
        let current_era = Self::current_era();

        let mut current_era_stash_snapshot = Self::eras_stash_points(current_era).unwrap();
        let mut next_era_stash_snapshot = Self::eras_stash_points(current_era + 1).unwrap();
        let mut current_era_machine_snapshot = Self::eras_machine_points(current_era).unwrap();
        let mut next_era_machine_snapshot = Self::eras_machine_points(current_era + 1).unwrap();

        let mut stash_machine = Self::stash_machines(&machine_info.machine_stash);
        let mut sys_info = Self::sys_info();

        let old_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);

        next_era_stash_snapshot.change_machine_online_status(
            machine_info.machine_stash.clone(),
            machine_info.machine_info_detail.committee_upload_info.gpu_num as u64,
            machine_info.machine_info_detail.committee_upload_info.calc_point,
            is_online,
        );

        if is_online {
            next_era_machine_snapshot.insert(
                machine_id.clone(),
                MachineGradeStatus {
                    basic_grade: machine_info.machine_info_detail.committee_upload_info.calc_point,
                    is_rented: false,
                },
            );

            ItemList::add_item(&mut stash_machine.online_machine, machine_id.clone());
            stash_machine.total_gpu_num += machine_base_info.gpu_num as u64;
            sys_info.total_gpu_num += machine_base_info.gpu_num as u64;
        } else {
            // NOTE: 24小时内，不能下线后再次下线。因为下线会清空当日得分记录，
            // 一天内再次下线会造成再次清空
            current_era_stash_snapshot.change_machine_online_status(
                machine_info.machine_stash.clone(),
                machine_info.machine_info_detail.committee_upload_info.gpu_num as u64,
                machine_info.machine_info_detail.committee_upload_info.calc_point,
                is_online,
            );
            current_era_machine_snapshot.remove(&machine_id);
            next_era_machine_snapshot.remove(&machine_id);

            ItemList::rm_item(&mut stash_machine.online_machine, &machine_id);
            stash_machine.total_gpu_num -= machine_base_info.gpu_num as u64;
            sys_info.total_gpu_num -= machine_base_info.gpu_num as u64;
        }

        // 机器上线或者下线都会影响下一era得分，而只有下线才影响当前era得分
        ErasStashPoints::<T>::insert(current_era + 1, next_era_stash_snapshot);
        ErasMachinePoints::<T>::insert(current_era + 1, next_era_machine_snapshot);
        if !is_online {
            ErasStashPoints::<T>::insert(current_era, current_era_stash_snapshot);
            ErasMachinePoints::<T>::insert(current_era, current_era_machine_snapshot);
        }

        let new_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);
        stash_machine.total_calc_points = stash_machine.total_calc_points + new_stash_grade - old_stash_grade;
        sys_info.total_calc_points = sys_info.total_calc_points + new_stash_grade - old_stash_grade;

        // NOTE: 5000张卡开启银河竞赛
        if !Self::galaxy_is_on() && sys_info.total_gpu_num > 5000 {
            GalaxyIsOn::<T>::put(true);
        }

        if is_online && stash_machine.online_machine.len() == 1 {
            sys_info.total_staker += 1;
        }
        if !is_online && stash_machine.online_machine.len() == 0 {
            sys_info.total_staker -= 1;
        }

        SysInfo::<T>::put(sys_info);
        StashMachines::<T>::insert(&machine_info.machine_stash, stash_machine);
    }

    // - Writes:
    // ErasStashPoints, ErasMachinePoints, SysInfo, StashMachines
    fn update_snap_by_rent_status(machine_id: MachineId, is_rented: bool) {
        let machine_info = Self::machines_info(&machine_id);
        let current_era = Self::current_era();

        let mut current_era_stash_snap = Self::eras_stash_points(current_era).unwrap_or_default();
        let mut next_era_stash_snap = Self::eras_stash_points(current_era + 1).unwrap_or_default();
        let mut current_era_machine_snap = Self::eras_machine_points(current_era).unwrap_or_default();
        let mut next_era_machine_snap = Self::eras_machine_points(current_era + 1).unwrap_or_default();

        let mut stash_machine = Self::stash_machines(&machine_info.machine_stash);
        let mut sys_info = Self::sys_info();

        let old_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);

        next_era_stash_snap.change_machine_rent_status(
            machine_info.machine_stash.clone(),
            machine_info.machine_info_detail.committee_upload_info.calc_point,
            is_rented,
        );
        next_era_machine_snap.insert(
            machine_id.clone(),
            MachineGradeStatus {
                basic_grade: machine_info.machine_info_detail.committee_upload_info.calc_point,
                is_rented,
            },
        );

        if !is_rented {
            current_era_stash_snap.change_machine_rent_status(
                machine_info.machine_stash.clone(),
                machine_info.machine_info_detail.committee_upload_info.calc_point,
                is_rented,
            );

            current_era_machine_snap.insert(
                machine_id.clone(),
                MachineGradeStatus {
                    basic_grade: machine_info.machine_info_detail.committee_upload_info.calc_point,
                    is_rented,
                },
            );
        }

        // 被租用或者退租都影响下一Era记录，而退租直接影响当前得分
        ErasStashPoints::<T>::insert(current_era + 1, next_era_stash_snap);
        ErasMachinePoints::<T>::insert(current_era + 1, next_era_machine_snap);
        if !is_rented {
            ErasStashPoints::<T>::insert(current_era, current_era_stash_snap);
            ErasMachinePoints::<T>::insert(current_era, current_era_machine_snap);
            sys_info.total_rented_gpu -= machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;
            stash_machine.total_rented_gpu -= machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;
        } else {
            sys_info.total_rented_gpu += machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;
            stash_machine.total_rented_gpu += machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;
        }

        let new_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);
        stash_machine.total_calc_points = stash_machine.total_calc_points + new_stash_grade - old_stash_grade;
        sys_info.total_calc_points = sys_info.total_calc_points + new_stash_grade - old_stash_grade;

        SysInfo::<T>::put(sys_info);
        StashMachines::<T>::insert(&machine_info.machine_stash, stash_machine);
    }

    // 根据机器得分快照，和委员会膨胀分数，计算应该奖励
    // end_era分发奖励
    fn distribute_reward() {
        let current_era = Self::current_era();
        let start_era = if current_era > 150 { current_era - 150 } else { 0u32 };
        let all_stash = Self::get_all_stash();

        for era_index in start_era..=current_era {
            let era_reward = Self::era_reward(era_index);
            let era_machine_points = Self::eras_machine_points(era_index).unwrap_or_default();
            let era_stash_points = Self::eras_stash_points(era_index).unwrap_or_default();

            for a_stash in &all_stash {
                let mut stash_machine = Self::stash_machines(a_stash);

                for machine_id in stash_machine.total_machine.clone() {
                    let _ = Self::distrubute_a_machine(
                        machine_id,
                        a_stash,
                        era_reward,
                        &era_stash_points,
                        &era_machine_points,
                        current_era,
                        era_index,
                        &mut stash_machine,
                    );
                }
                StashMachines::<T>::insert(a_stash, stash_machine);
            }
        }
    }

    fn distrubute_a_machine(
        machine_id: MachineId,
        a_stash: &T::AccountId,
        era_reward: BalanceOf<T>,
        era_stash_points: &EraStashPoints<T::AccountId>,
        era_machine_points: &BTreeMap<MachineId, MachineGradeStatus>,
        current_era: EraIndex,
        era_index: EraIndex,
        stash_machine: &mut StashMachine<BalanceOf<T>>,
    ) -> Result<(), ()> {
        let machine_info = Self::machines_info(&machine_id);
        let will_reward_committee = machine_info.reward_deadline >= current_era;
        let is_head_mining = era_index == current_era;

        // 计算当时机器实际获得的奖励
        let machine_points = era_machine_points.get(&machine_id).ok_or(())?;
        let stash_points = era_stash_points.staker_statistic.get(&a_stash).ok_or(())?;
        let machine_actual_grade = machine_points.machine_actual_grade(stash_points.inflation);

        // 该Era机器获得的总奖励
        let machine_total_reward =
            Perbill::from_rational_approximation(machine_actual_grade, era_stash_points.total) * era_reward;
        let linear_reward_part = Perbill::from_rational_approximation(75u32, 100u32) * machine_total_reward;

        // 根据是否是第一次释放奖励，计算era_index时奖励数量: 第一次释放25%, 否则释放剩余75%的1/150
        let release_now = if is_head_mining {
            machine_total_reward.checked_sub(&linear_reward_part).ok_or(())?
        } else {
            Perbill::from_rational_approximation(1u32, 150u32) * linear_reward_part
        };

        // 如果奖励不发给委员会
        if !will_reward_committee {
            if is_head_mining {
                // 更新一些数据
                stash_machine.total_earned_reward =
                    stash_machine.total_earned_reward.checked_add(&machine_total_reward).ok_or(())?;
                ErasMachineReward::<T>::insert(current_era, &machine_id, machine_total_reward);
                ErasStashReward::<T>::mutate(&current_era, &a_stash, |old_value| *old_value += machine_total_reward);
            }
            // 发放奖励
            // 没有委员会来分，则全部奖励给stash账户
            stash_machine.can_claim_reward = stash_machine.can_claim_reward.checked_add(&release_now).ok_or(())?;
            ErasMachineReleasedReward::<T>::mutate(&current_era, &machine_id, |old_value| *old_value += release_now);
            ErasStashReleasedReward::<T>::mutate(&current_era, &machine_info.machine_stash, |old_value| {
                *old_value += release_now
            });
        } else {
            // 头矿时更新记录
            if is_head_mining {
                // 如果委员的奖励时间会很快就要结束了
                // 则奖励的前一部分给委员会一部分，后一部分，不给委员会
                if machine_info.reward_deadline >= current_era + 150 {
                    let reward_to_stash = Perbill::from_rational_approximation(99u64, 100u64) * machine_total_reward;
                    stash_machine.total_earned_reward =
                        stash_machine.total_earned_reward.checked_add(&reward_to_stash).ok_or(())?;
                    ErasMachineReward::<T>::insert(current_era, &machine_id, reward_to_stash);
                    ErasStashReward::<T>::mutate(&current_era, &machine_info.machine_stash, |old_value| {
                        *old_value += reward_to_stash;
                    });
                } else {
                    // 计算委员会奖励结束后，机器拥有者单独获得的奖励
                    let only_reward_to_stash_duration = current_era + 150 - machine_info.reward_deadline;
                    let reward_to_stash2 =
                        Perbill::from_rational_approximation(only_reward_to_stash_duration, 150) * machine_total_reward;

                    // 计算委员会奖励结束前，机器拥有者单独获得的奖励
                    let reward_to_both = machine_total_reward - reward_to_stash2;
                    let reward_to_stash1 = Perbill::from_rational_approximation(99u32, 100u32) * reward_to_both;

                    let stash_all_get = reward_to_stash1 + reward_to_stash2;
                    stash_machine.total_earned_reward =
                        stash_machine.total_earned_reward.checked_add(&stash_all_get).ok_or(())?;

                    ErasMachineReward::<T>::insert(current_era, &machine_id, stash_all_get);
                    ErasStashReward::<T>::mutate(&current_era, &machine_info.machine_stash, |old_value| {
                        *old_value += stash_all_get
                    });
                }
            }

            // 99% 分给stash账户
            let release_to_stash = Perbill::from_rational_approximation(99u64, 100u64) * release_now;
            stash_machine.can_claim_reward += release_to_stash;

            ErasMachineReleasedReward::<T>::mutate(&current_era, &machine_id, |old_value| {
                *old_value += release_to_stash
            });
            ErasStashReleasedReward::<T>::mutate(&current_era, &machine_info.machine_stash, |old_value| {
                *old_value += release_to_stash
            });

            // 剩下分给committee
            let release_to_committee = release_now - release_to_stash;
            let committee_each_get =
                Perbill::from_rational_approximation(1u64, machine_info.reward_committee.len() as u64) *
                    release_to_committee;
            for a_committee in machine_info.reward_committee.clone() {
                T::ManageCommittee::add_reward(a_committee, committee_each_get);
            }
        }

        Ok(())
    }
}

/// 审查委员会可以执行的操作
impl<T: Config> OCOps for Pallet<T> {
    type MachineId = MachineId;
    type AccountId = T::AccountId;
    type CommitteeUploadInfo = CommitteeUploadInfo;

    // 委员会订阅了一个机器ID
    // 将机器状态从ocw_confirmed_machine改为booked_machine，同时将机器状态改为booked
    // - Writes: LiveMachine, MachinesInfo
    fn oc_booked_machine(id: MachineId) {
        let mut live_machines = Self::live_machines();

        ItemList::rm_item(&mut live_machines.confirmed_machine, &id);
        ItemList::add_item(&mut live_machines.booked_machine, id.clone());

        let mut machine_info = Self::machines_info(&id);
        machine_info.machine_status = MachineStatus::CommitteeVerifying;

        LiveMachines::<T>::put(live_machines);
        MachinesInfo::<T>::insert(&id, machine_info);
    }

    // 由于委员会没有达成一致，需要重新返回到bonding_machine
    fn oc_revert_booked_machine(id: MachineId) {
        let mut live_machines = Self::live_machines();

        ItemList::rm_item(&mut live_machines.booked_machine, &id);
        ItemList::add_item(&mut live_machines.confirmed_machine, id.clone());

        let mut machine_info = Self::machines_info(&id);
        machine_info.machine_status = MachineStatus::DistributingOrder;

        LiveMachines::<T>::put(live_machines);
        MachinesInfo::<T>::insert(&id, machine_info);
    }

    // 当多个委员会都对机器进行了确认之后，添加机器信息，并更新机器得分
    // 机器被成功添加, 则添加上可以获取收益的委员会
    fn oc_confirm_machine(
        reported_committee: Vec<T::AccountId>,
        committee_upload_info: CommitteeUploadInfo,
    ) -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let current_era = Self::current_era();

        let mut machine_info = Self::machines_info(&committee_upload_info.machine_id);
        let mut live_machines = Self::live_machines();

        let is_reonline =
            UserReonlineStake::<T>::contains_key(&machine_info.machine_stash, &committee_upload_info.machine_id);

        ItemList::rm_item(&mut live_machines.booked_machine, &committee_upload_info.machine_id);

        machine_info.machine_info_detail.committee_upload_info = committee_upload_info.clone();

        if !is_reonline {
            machine_info.reward_committee = reported_committee.clone();
        }

        // 改变用户的绑定数量。如果用户余额足够，则直接质押。否则将机器状态改为补充质押
        let stake_need = machine_info
            .init_stake_per_gpu
            .checked_mul(&committee_upload_info.gpu_num.saturated_into::<BalanceOf<T>>())
            .ok_or(())?;
        if let Some(extra_stake) = stake_need.checked_sub(&machine_info.stake_amount) {
            if Self::change_user_total_stake(machine_info.machine_stash.clone(), extra_stake, true).is_ok() {
                ItemList::add_item(&mut live_machines.online_machine, committee_upload_info.machine_id.clone());
                machine_info.stake_amount = stake_need;
                machine_info.machine_status = MachineStatus::Online;
                machine_info.last_online_height = now;
                machine_info.last_machine_restake = now;

                if !is_reonline {
                    machine_info.online_height = now;
                    machine_info.reward_deadline = current_era + REWARD_DURATION;
                }
            } else {
                ItemList::add_item(&mut live_machines.fulfilling_machine, committee_upload_info.machine_id.clone());
                machine_info.machine_status = MachineStatus::WaitingFulfill;
            }
        } else {
            ItemList::add_item(&mut live_machines.online_machine, committee_upload_info.machine_id.clone());
            machine_info.machine_status = MachineStatus::Online;
            if !is_reonline {
                machine_info.reward_deadline = current_era + REWARD_DURATION;
            }
        }

        MachinesInfo::<T>::insert(committee_upload_info.machine_id.clone(), machine_info.clone());
        LiveMachines::<T>::put(live_machines);

        if is_reonline {
            // 根据质押，奖励给这些委员会
            let reonline_stake =
                Self::user_reonline_stake(&machine_info.machine_stash, &committee_upload_info.machine_id);
            let _ = Self::reward_reonline_committee(
                machine_info.machine_stash.clone(),
                reonline_stake.stake_amount,
                reported_committee,
            );
        }

        // NOTE: Must be after MachinesInfo change, which depend on machine_info
        if let MachineStatus::Online = machine_info.machine_status {
            Self::change_pos_gpu_by_online(&committee_upload_info.machine_id, true);
            Self::update_snap_by_online_status(committee_upload_info.machine_id.clone(), true);

            if is_reonline {
                // 仅在Oline成功时删掉reonline_stake记录，以便补充质押时惩罚时检查状态
                let reonline_stake =
                    Self::user_reonline_stake(&machine_info.machine_stash, &committee_upload_info.machine_id);

                UserReonlineStake::<T>::remove(&machine_info.machine_stash, &committee_upload_info.machine_id);

                // 惩罚该机器，如果机器是Fulfill，则等待Fulfill之后，再进行惩罚
                let offline_duration = now - reonline_stake.offline_time;
                Self::slash_when_report_offline(
                    committee_upload_info.machine_id.clone(),
                    OPSlashReason::OnlineReportOffline(offline_duration),
                    None,
                    None,
                );
            }
        }

        return Ok(())
    }

    // When committees reach an agreement to refuse machine, change machine status and record refuse time
    fn oc_refuse_machine(machine_id: MachineId, committee: Vec<T::AccountId>) -> Result<(), ()> {
        // Refuse controller bond machine, and clean storage
        let machine_info = Self::machines_info(&machine_id);
        let mut live_machines = Self::live_machines();
        let mut stash_stake = Self::stash_stake(&machine_info.machine_stash);

        // In case this offline is for change hardware info, when reonline is refused, reward to committee and
        // machine info should not be deleted
        let is_mut_hardware = live_machines.refused_mut_hardware_machine.binary_search(&machine_id).is_ok();
        if is_mut_hardware {
            let reonline_stake = Self::user_reonline_stake(&machine_info.machine_stash, &machine_id);
            let now = <frame_system::Module<T>>::block_number();

            let slash_id = Self::get_new_slash_id();
            let slash_info = OPPendingSlashInfo {
                slash_who: machine_info.machine_stash,
                machine_id: machine_id.clone(),
                slash_time: now,
                slash_amount: reonline_stake.stake_amount,
                slash_exec_time: now + (2880u32 * 2).saturated_into::<T::BlockNumber>(),
                reward_to_reporter: None,
                reward_to_committee: Some(committee),
                slash_reason: OPSlashReason::CommitteeRefusedMutHardware,
            };
            PendingSlash::<T>::insert(slash_id, slash_info);

            ItemList::rm_item(&mut live_machines.refused_mut_hardware_machine, &machine_id);
            ItemList::add_item(&mut live_machines.bonding_machine, machine_id.clone());

            LiveMachines::<T>::put(live_machines);
            return Ok(())
        }

        let now = <frame_system::Module<T>>::block_number();
        let mut sys_info = Self::sys_info();
        let mut stash_machines = Self::stash_machines(&machine_info.machine_stash);
        let mut controller_machines = Self::controller_machines(&machine_info.controller);

        sys_info.total_stake = sys_info.total_stake.checked_sub(&machine_info.stake_amount).ok_or(())?;

        // Slash 5% of init stake(5% of one gpu stake)
        let slash = Perbill::from_rational_approximation(5u64, 100u64) * machine_info.stake_amount;
        let left_stake = machine_info.stake_amount.checked_sub(&slash).ok_or(())?;

        // Return 95% left stake(95% of one gpu stake)
        <T as pallet::Config>::Currency::unreserve(&machine_info.machine_stash, left_stake);
        stash_stake = stash_stake.checked_sub(&left_stake).ok_or(())?;

        // Add a pending slash
        let slash_id = Self::get_new_slash_id();
        let slash_info = OPPendingSlashInfo {
            slash_who: machine_info.machine_stash.clone(),
            machine_id: machine_id.clone(),
            slash_time: now,
            slash_amount: slash,
            slash_exec_time: now + (2880u32 * 2).saturated_into::<T::BlockNumber>(),
            reward_to_reporter: None,
            reward_to_committee: None,
            slash_reason: OPSlashReason::CommitteeRefusedOnline,
        };

        // Clean storage
        ItemList::rm_item(&mut controller_machines, &machine_id);
        ItemList::rm_item(&mut stash_machines.total_machine, &machine_id);

        let mut live_machines = Self::live_machines();
        ItemList::rm_item(&mut live_machines.booked_machine, &machine_id);
        ItemList::add_item(&mut live_machines.refused_machine, machine_id.clone());

        StashStake::<T>::insert(&machine_info.machine_stash, stash_stake);
        LiveMachines::<T>::put(live_machines);
        PendingSlash::<T>::insert(slash_id, slash_info);
        MachinesInfo::<T>::remove(&machine_id);
        ControllerMachines::<T>::insert(&machine_info.controller, controller_machines);
        StashMachines::<T>::insert(&machine_info.machine_stash, stash_machines);
        SysInfo::<T>::put(sys_info);

        Ok(())
    }
}

impl<T: Config> RTOps for Pallet<T> {
    type MachineId = MachineId;
    type MachineStatus = MachineStatus<T::BlockNumber, T::AccountId>;
    type AccountId = T::AccountId;
    type BalanceOf = BalanceOf<T>;

    fn change_machine_status(
        machine_id: &MachineId,
        new_status: MachineStatus<T::BlockNumber, T::AccountId>,
        renter: Option<Self::AccountId>,
        rent_duration: Option<u64>,
    ) {
        let mut machine_info = Self::machines_info(machine_id);
        let mut live_machines = Self::live_machines();

        machine_info.machine_status = new_status.clone();
        machine_info.last_machine_renter = renter;

        match new_status {
            MachineStatus::Rented => {
                machine_info.total_rented_times += 1;
                // 机器创建成功
                Self::update_snap_by_rent_status(machine_id.to_vec(), true);

                ItemList::rm_item(&mut live_machines.online_machine, &machine_id);
                ItemList::add_item(&mut live_machines.rented_machine, machine_id.clone());
                LiveMachines::<T>::put(live_machines);

                Self::change_pos_gpu_by_rent(machine_id, true);
            },
            // 租用结束 或 租用失败(半小时无确认)
            MachineStatus::Online => {
                if rent_duration.is_some() {
                    machine_info.total_rented_duration += rent_duration.unwrap_or_default();
                    // 租用结束
                    Self::update_snap_by_rent_status(machine_id.to_vec(), false);

                    ItemList::rm_item(&mut live_machines.rented_machine, &machine_id);
                    ItemList::add_item(&mut live_machines.online_machine, machine_id.clone());
                    LiveMachines::<T>::put(live_machines);

                    Self::change_pos_gpu_by_rent(machine_id, false);
                }
            },
            _ => {},
        }

        MachinesInfo::<T>::insert(&machine_id, machine_info);
    }

    fn change_machine_rent_fee(amount: BalanceOf<T>, machine_id: MachineId, is_burn: bool) {
        let mut machine_info = Self::machines_info(&machine_id);
        let mut staker_machine = Self::stash_machines(&machine_info.machine_stash);
        let mut sys_info = Self::sys_info();
        if is_burn {
            machine_info.total_burn_fee += amount;
            staker_machine.total_burn_fee += amount;
            sys_info.total_burn_fee += amount;
        } else {
            machine_info.total_rent_fee += amount;
            staker_machine.total_rent_fee += amount;
            sys_info.total_rent_fee += amount;
        }
        SysInfo::<T>::put(sys_info);
        StashMachines::<T>::insert(&machine_info.machine_stash, staker_machine);
        MachinesInfo::<T>::insert(&machine_id, machine_info);
    }
}

impl<T: Config> OPRPCQuery for Pallet<T> {
    type AccountId = T::AccountId;
    type StashMachine = StashMachine<BalanceOf<T>>;

    fn get_all_stash() -> Vec<T::AccountId> {
        <StashMachines<T> as IterableStorageMap<T::AccountId, _>>::iter()
            .map(|(staker, _)| staker)
            .collect::<Vec<_>>()
    }

    fn get_stash_machine(stash: T::AccountId) -> StashMachine<BalanceOf<T>> {
        Self::stash_machines(stash)
    }
}

impl<T: Config> MTOps for Pallet<T> {
    type MachineId = MachineId;
    type AccountId = T::AccountId;
    type FaultType = OPSlashReason<T::BlockNumber>;

    fn mt_machine_offline(
        reporter: T::AccountId,
        committee: Vec<T::AccountId>,
        machine_id: MachineId,
        fault_type: OPSlashReason<T::BlockNumber>,
    ) {
        let machine_info = Self::machines_info(&machine_id);

        Self::machine_offline(
            machine_id,
            MachineStatus::ReporterReportOffline(
                fault_type,
                Box::new(machine_info.machine_status),
                reporter,
                committee,
            ),
        );
    }
}

// RPC
impl<T: Config> Module<T> {
    pub fn get_total_staker_num() -> u64 {
        let all_stash = Self::get_all_stash();
        return all_stash.len() as u64
    }

    pub fn get_op_info() -> SysInfoDetail<BalanceOf<T>> {
        Self::sys_info()
    }

    pub fn get_staker_info(
        account: impl EncodeLike<T::AccountId>,
    ) -> RpcStakerInfo<BalanceOf<T>, T::BlockNumber, T::AccountId> {
        let staker_info = Self::stash_machines(account);

        let mut staker_machines = Vec::new();

        for a_machine in &staker_info.total_machine {
            let machine_info = Self::machines_info(a_machine);
            staker_machines.push(rpc_types::MachineBriefInfo {
                machine_id: a_machine.to_vec(),
                gpu_num: machine_info.machine_info_detail.committee_upload_info.gpu_num,
                calc_point: machine_info.machine_info_detail.committee_upload_info.calc_point,
                machine_status: machine_info.machine_status,
            })
        }

        RpcStakerInfo { stash_statistic: staker_info, bonded_machines: staker_machines }
    }

    /// 获取机器列表
    pub fn get_machine_list() -> LiveMachine {
        Self::live_machines()
    }

    /// 获取机器详情
    pub fn get_machine_info(machine_id: MachineId) -> MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        Self::machines_info(&machine_id)
    }

    /// 获得系统中所有位置列表
    pub fn get_pos_gpu_info() -> Vec<(Longitude, Latitude, PosInfo)> {
        <PosGPUInfo<T> as IterableStorageDoubleMap<Longitude, Latitude, PosInfo>>::iter()
            .map(|(k1, k2, v)| (k1, k2, v))
            .collect()
    }

    /// 获得某个机器某个Era奖励数量
    pub fn get_machine_era_reward(machine_id: MachineId, era_index: EraIndex) -> BalanceOf<T> {
        Self::eras_machine_reward(era_index, machine_id)
    }

    /// 获得某个机器某个Era实际奖励数量
    pub fn get_machine_era_released_reward(machine_id: MachineId, era_index: EraIndex) -> BalanceOf<T> {
        Self::eras_machine_released_reward(era_index, machine_id)
    }

    /// 获得某个Stash账户某个Era获得的奖励数量
    pub fn get_stash_era_reward(stash: T::AccountId, era_index: EraIndex) -> BalanceOf<T> {
        Self::eras_stash_reward(era_index, stash)
    }

    /// 获得某个Stash账户某个Era实际解锁的奖励数量
    pub fn get_stash_era_released_reward(stash: T::AccountId, era_index: EraIndex) -> BalanceOf<T> {
        Self::eras_stash_released_reward(era_index, stash)
    }
}
