//! 机器单卡质押数量：
//!   n = min(n1, n2)
//!   n1 = 5w RMB 等值DBC
//!   n2 = 100000 DBC (卡数 <= 10000)
//!   n2 = 100000 * (10000/卡数) (卡数>10000)
//! 在线奖励数量:
//!   1th month:     10^8;                   Phase0  30 day
//!   Next 2 year:   4 * 10^8;               Phase1  730 day
//!   Next 9 month:  4 * 10^8 * (9 / 12);    Phase2  270 day
//!   Next 5 year:   5 * 10^7;               Phase3  1825 day
//!   Next 5 years:  2.5 * 10^7;             Phase4  1825 day
//! 机器得分如何计算：
//!   机器相对标准配置得到算力点数。机器实际得分 = 算力点数 + 算力点数 * 集群膨胀系数 + 算力点数 * 30%
//!   因此，机器被租用时，机器实际得分 = 算力点数 * (1 + 集群膨胀系数 + 30%租用膨胀系数)
//!  在线奖励释放：25%立即释放,剩余75%线性释放时间长度

#![cfg_attr(not(feature = "std"), no_std)]

use codec::EncodeLike;
use frame_support::{
    dispatch::DispatchResultWithPostInfo,
    pallet_prelude::*,
    traits::{Currency, ExistenceRequirement::KeepAlive, Get, LockIdentifier, LockableCurrency, WithdrawReasons},
    weights::Weight,
    IterableStorageDoubleMap, IterableStorageMap,
};
use frame_system::pallet_prelude::*;
use online_profile_machine::{DbcPrice, LCOps, MTOps, ManageCommittee, OPRPCQuery, RTOps};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::{crypto::Public, H256};
use sp_runtime::{
    traits::{CheckedAdd, CheckedMul, CheckedSub, Verify},
    Perbill, SaturatedConversion,
};
use sp_std::{
    collections::btree_map::BTreeMap,
    convert::{TryFrom, TryInto},
    prelude::*,
    str,
    vec::Vec,
};

pub mod op_types;
pub mod rpc_types;

pub use op_types::*;
pub use rpc_types::*;

pub use pallet::*;

/// 每个Era有多少个Block
pub const BLOCK_PER_ERA: u64 = 2880;
pub const REWARD_DURATION: u32 = 365 * 2;

pub const PALLET_LOCK_ID: LockIdentifier = *b"olprofil";

// #[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
// pub struct MachineStakeParams<Balance> {}

// #[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
// pub struct MachineStake<Balance> {}

/// stash账户总览自己当前状态
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct StashMachine<Balance> {
    /// stash账户绑定的所有机器，不与机器状态有关，下线的机器150天后，奖励释放完才被删除。
    pub total_machine: Vec<MachineId>,
    /// stash账户绑定的处于在线状态的机器
    pub online_machine: Vec<MachineId>,
    /// 在线机器总得分，集群膨胀系数与在线奖励需要**计算在内**
    pub total_calc_points: u64,
    /// 在线机器的总GPU个数
    pub total_gpu_num: u64,
    /// 被租用的GPU个数
    pub total_rented_gpu: u64,
    /// 总计获取的奖励,包含锁定的奖励
    pub total_earned_reward: Balance,
    /// 总计领取奖励数量
    pub total_claimed_reward: Balance,
    /// 目前能够领取奖励的数量
    pub can_claim_reward: Balance,
    /// 总租金收益(银河竞赛前获得)
    pub total_rent_fee: Balance,
    /// 总销毁数量(银河竞赛后销毁)
    pub total_burn_fee: Balance,
}

/// 机器的信息
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineInfo<AccountId: Ord, BlockNumber, Balance> {
    /// 绑定机器的人
    pub controller: AccountId,
    /// 奖励发放账户(机器内置钱包地址)
    pub machine_stash: AccountId,
    /// 最近的机器的租用者
    pub last_machine_renter: Option<AccountId>,
    /// 最后一次重新质押时间(每365天允许重新质押一次)
    pub last_machine_restake: BlockNumber,
    /// 记录机器第一次绑定上线的时间
    pub bonding_height: BlockNumber,
    /// 机器第一次Online时间
    pub online_height: BlockNumber,
    /// 机器最近一次上线时间，当从Rented变为Online，也许要改变该变量
    pub last_online_height: BlockNumber,
    /// 该机器质押数量
    pub stake_amount: Balance,
    /// 机器的状态
    pub machine_status: MachineStatus<BlockNumber>,
    /// 总租用累计时长
    pub total_rented_duration: u64,
    /// 总租用次数
    pub total_rented_times: u64,
    /// 总租金收益(银河竞赛前获得)
    pub total_rent_fee: Balance,
    /// 总销毁数量
    pub total_burn_fee: Balance,
    /// 委员会提交的机器信息与用户自定义的信息
    pub machine_info_detail: MachineInfoDetail,
    /// 列表中的委员将分得用户每天奖励的1%
    pub reward_committee: Vec<AccountId>,
    /// 列表中委员分得奖励结束时间
    pub reward_deadline: EraIndex,
}

/// 机器状态
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum MachineStatus<BlockNumber> {
    /// 执行bond操作后，等待提交自定义信息
    AddingCustomizeInfo,
    /// 正在等待派单
    DistributingOrder,
    /// 派单后正在进行验证
    CommitteeVerifying,
    /// 委员会拒绝机器上线
    CommitteeRefused(BlockNumber),
    /// 补交质押
    WaitingFulfill,
    /// 正在上线，且未被租用
    Online,
    /// 机器管理者报告机器已下线
    StakerReportOffline(BlockNumber, Box<Self>),
    /// 报告人报告机器下线
    ReporterReportOffline(BlockNumber, Box<Self>),
    // ReporterReportOffline(BlockNumber, Box<Self>), // TODO
    /// 机器被租用，虚拟机正在被创建，等待用户提交机器创建完成的信息
    Creating,
    /// 已经被租用
    Rented,
}

impl<BlockNumber> Default for MachineStatus<BlockNumber> {
    fn default() -> Self {
        MachineStatus::AddingCustomizeInfo
    }
}

/// 算工被惩罚的原因
pub enum StashSlashReason<BlockNumber> {
    /// 主动报告被租用的机器下线
    RentedReportOffline(BlockNumber),
    /// 主动报告在线的机器下线
    OnlineReportOffline(BlockNumber),
    /// 机器被租用，但被举报有无法访问的故障
    RentedInaccessible(BlockNumber),
    /// 机器被租用，但被举报有硬件故障
    RentedHardwareMalfunction(BlockNumber),
    /// 机器被租用，但被举报硬件参数造假
    RentedHardwareCounterfeit(BlockNumber),
    /// 机器是在线状态，但无法租用
    OnlineRentFailed(BlockNumber),
}

// FIXME: bug
// impl<BlockNumber> Default for StashSlashReason<BlockNumber> {
//     fn default() -> Self {
//         StashSlashReason::OnlineReportOffline(0u32.saturated_into::<BlockNumber>())
//     }
// }

// 只保存正常声明周期的Machine,删除掉的/绑定失败的不保存在该变量中
/// 系统中存在的机器列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct LiveMachine {
    /// 用户质押DBC并绑定机器，机器等待控制人提交信息
    pub bonding_machine: Vec<MachineId>,
    /// 补交了自定义信息的机器，机器等待分派委员会
    pub confirmed_machine: Vec<MachineId>,
    /// 当机器已经全部分配了委员会。若lc确认机器失败(认可=不认可时)则返回上一状态，重新分派订单
    pub booked_machine: Vec<MachineId>,
    /// 委员会确认之后，机器上线
    pub online_machine: Vec<MachineId>,
    /// 委员会同意上线，但是由于stash账户质押不够，需要补充质押
    pub fulfilling_machine: Vec<MachineId>,
    /// 被委员会拒绝的机器（10天内还能重新申请上线）
    pub refused_machine: Vec<MachineId>,
    /// 被用户租用的机器，当机器被租用时，从online_machine中移除
    pub rented_machine: Vec<MachineId>,
    /// 下线的机器
    pub offline_machine: Vec<MachineId>,
}

impl LiveMachine {
    /// Check if machine_id exist
    fn machine_id_exist(&self, machine_id: &MachineId) -> bool {
        if self.bonding_machine.binary_search(machine_id).is_ok() ||
            self.confirmed_machine.binary_search(machine_id).is_ok() ||
            self.booked_machine.binary_search(machine_id).is_ok() ||
            self.online_machine.binary_search(machine_id).is_ok() ||
            self.fulfilling_machine.binary_search(machine_id).is_ok() ||
            self.refused_machine.binary_search(machine_id).is_ok()
        {
            return true
        }
        false
    }

    /// Add machine_id to one field of LiveMachine
    fn add_machine_id(a_field: &mut Vec<MachineId>, machine_id: MachineId) {
        if let Err(index) = a_field.binary_search(&machine_id) {
            a_field.insert(index, machine_id);
        }
    }

    /// Delete machine_id from one field of LiveMachine
    fn rm_machine_id(a_field: &mut Vec<MachineId>, machine_id: &MachineId) {
        if let Ok(index) = a_field.binary_search(machine_id) {
            a_field.remove(index);
        }
    }
}

/// Standard GPU rent price Per Era
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct StandardGpuPointPrice {
    /// Standard GPU calc points
    pub gpu_point: u64,
    /// Standard GPU price
    pub gpu_price: u64,
}

type BalanceOf<T> = <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// SysInfo of onlineProfile pallet
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct SysInfoDetail<Balance> {
    /// 在线机器的GPU的总数
    pub total_gpu_num: u64,
    /// 被租用机器的GPU的总数
    pub total_rented_gpu: u64,
    /// 系统中总stash账户数量(有机器成功上线)
    pub total_staker: u64,
    /// 系统中上线的总算力点数, 考虑额外得分
    pub total_calc_points: u64,
    /// 系统中DBC质押总数
    pub total_stake: Balance,
    /// 系统中产生的租金收益总数(银河竞赛开启前)
    pub total_rent_fee: Balance,
    /// 系统中租金销毁总数(银河竞赛开启后)
    pub total_burn_fee: Balance,
}

/// 不同经纬度GPU信息统计
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct PosInfo {
    /// 在线机器的GPU数量
    pub online_gpu: u64,
    /// 离线GPU数量
    pub offline_gpu: u64,
    /// 被租用机器GPU数量
    pub rented_gpu: u64,
    // FIXME: 膨胀得分考虑在内
    /// 在线机器算力点数
    pub online_gpu_calc_points: u64,
}

/// 资金账户对每台机器的质押情况
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct StashMachineStakeInfo<Balance> {
    staked_amount: Balance,
    used_stake: Balance,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + dbc_price_ocw::Config + generic_func::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type BondingDuration: Get<EraIndex>;
        type DbcPrice: DbcPrice<BalanceOf = BalanceOf<Self>>;
        type ManageCommittee: ManageCommittee<AccountId = Self::AccountId, BalanceOf = BalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    /// 机器单卡质押数量，单位DBC。如100_000DBC
    #[pallet::storage]
    #[pallet::getter(fn stake_per_gpu)]
    pub(super) type StakePerGPU<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    /// 标准显卡算力点数和租用价格(USD*10^6/Era)
    #[pallet::storage]
    #[pallet::getter(fn standard_gpu_point_price)]
    pub(super) type StandardGPUPointPrice<T: Config> = StorageValue<_, StandardGpuPointPrice>;

    /// 单卡质押上限。USD*10^6
    #[pallet::storage]
    #[pallet::getter(fn stake_usd_limit)]
    pub(super) type StakeUSDLimit<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 机器重新上线需要的手续费。USD*10^6，默认300RMB等值
    #[pallet::storage]
    #[pallet::getter(fn reonline_stake)]
    pub(super) type ReonlineStake<T: Config> = StorageValue<_, u64, ValueQuery>;

    /// 重新上线用户质押的数量
    #[pallet::storage]
    #[pallet::getter(fn user_reonline_stake)]
    pub(super) type UserReonlineStake<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, MachineId, BalanceOf<T>, ValueQuery>;

    /// 银河竞赛是否开启。5000张卡自动开启
    #[pallet::storage]
    #[pallet::getter(fn galaxy_is_on)]
    pub(super) type GalaxyIsOn<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// 模块统计信息
    #[pallet::storage]
    #[pallet::getter(fn sys_info)]
    pub(super) type SysInfo<T: Config> = StorageValue<_, SysInfoDetail<BalanceOf<T>>, ValueQuery>;

    /// 不同经纬度GPU信息统计
    #[pallet::storage]
    #[pallet::getter(fn pos_gpu_info)]
    pub(super) type PosGPUInfo<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, Longitude, Blake2_128Concat, Latitude, PosInfo, ValueQuery>;

    /// stash 对应的 controller
    #[pallet::storage]
    #[pallet::getter(fn stash_controller)]
    pub(super) type StashController<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    /// controller 控制的 stash
    #[pallet::storage]
    #[pallet::getter(fn controller_stash)]
    pub(super) type ControllerStash<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    /// 机器的详细信息
    #[pallet::storage]
    #[pallet::getter(fn machines_info)]
    pub type MachinesInfo<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>, ValueQuery>;

    /// stash账户下所有机器统计
    #[pallet::storage]
    #[pallet::getter(fn stash_machines)]
    pub(super) type StashMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, StashMachine<BalanceOf<T>>, ValueQuery>;

    /// stash账户下的所有机房信息
    #[pallet::storage]
    #[pallet::getter(fn stash_server_rooms)]
    pub(super) type StashServerRooms<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Vec<H256>, ValueQuery>;

    /// 某机房下的所有机器
    #[pallet::storage]
    #[pallet::getter(fn server_room_machines)]
    pub(super) type ServerRoomMachines<T: Config> = StorageMap<_, Blake2_128Concat, H256, Vec<MachineId>>;

    /// controller账户下的所有机器
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
        StorageMap<_, Blake2_128Concat, EraIndex, BTreeMap<MachineId, MachineGradeStatus<T::AccountId>>>;

    /// 在线奖励开始时间
    #[pallet::storage]
    #[pallet::getter(fn reward_start_era)]
    pub(super) type RewardStartEra<T: Config> = StorageValue<_, EraIndex>;

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

    /// 不同阶段不同奖励
    #[pallet::storage]
    #[pallet::getter(fn phase_n_reward_per_era)]
    pub(super) type PhaseNRewardPerEra<T: Config> = StorageMap<_, Blake2_128Concat, u32, BalanceOf<T>>;

    /// 资金账户对每台机器的质押状况
    #[pallet::storage]
    #[pallet::getter(fn stash_machine_stake)]
    pub(super) type StashMachineStake<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, MachineId, BalanceOf<T>, ValueQuery>;

    /// 资金账户的质押总计
    #[pallet::storage]
    #[pallet::getter(fn stash_stake)]
    pub(super) type StashStake<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

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
                    let init_value: BTreeMap<MachineId, MachineGradeStatus<T::AccountId>> = BTreeMap::new();
                    ErasMachinePoints::<T>::insert(0, init_value.clone());
                    ErasMachinePoints::<T>::insert(1, init_value);
                } else {
                    // 用当前的Era快照初始化下一个Era的信息
                    let current_era_stash_snapshot = Self::eras_stash_points(current_era).unwrap();
                    ErasStashPoints::<T>::insert(current_era + 1, current_era_stash_snapshot);
                    let current_era_machine_snapshot = Self::eras_machine_points(current_era).unwrap();
                    ErasMachinePoints::<T>::insert(current_era + 1, current_era_machine_snapshot);
                }
            }
            0
        }

        fn on_finalize(block_number: T::BlockNumber) {
            let current_height = block_number.saturated_into::<u64>();

            // 在每个Era结束时执行奖励，发放到用户的Machine
            // 计算奖励，直接根据当前得分即可
            if current_height > 0 && current_height % BLOCK_PER_ERA == 0 {
                if Self::distribute_reward().is_err() {
                    debug::error!("Failed to distribute reward");
                }
            }

            Self::clean_refused_machine();
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 实现当达到5000卡时，开启奖励
        #[pallet::weight(0)]
        pub fn set_reward_start_era(origin: OriginFor<T>, reward_start_era: EraIndex) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            RewardStartEra::<T>::put(reward_start_era);
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn set_phase_n_reward_per_era(
            origin: OriginFor<T>,
            phase: u32,
            reward_per_era: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            match phase {
                0..=5 => PhaseNRewardPerEra::<T>::insert(phase, reward_per_era),
                _ => return Err(Error::<T>::RewardPhaseOutOfRange.into()),
            }
            Ok(().into())
        }

        /// 设置单卡质押DBC数量
        #[pallet::weight(0)]
        pub fn set_gpu_stake(origin: OriginFor<T>, stake_per_gpu: BalanceOf<T>) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StakePerGPU::<T>::put(stake_per_gpu);
            Ok(().into())
        }

        /// 单GPU质押量等价USD的上限
        #[pallet::weight(0)]
        pub fn set_stake_usd_limit(origin: OriginFor<T>, stake_usd_limit: u64) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StakeUSDLimit::<T>::put(stake_usd_limit);
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

        #[pallet::weight(0)]
        pub fn add_era_reward(
            origin: OriginFor<T>,
            era_index: EraIndex,
            reward: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            EraReward::<T>::insert(era_index, reward);
            Ok(().into())
        }

        /// 上线被拒绝后，重新上线需要质押
        #[pallet::weight(0)]
        pub fn set_reonline_stake(origin: OriginFor<T>, reonline_stake: u64) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            ReonlineStake::<T>::put(reonline_stake);
            Ok(().into())
        }

        /// stash账户设置一个控制账户
        #[pallet::weight(10000)]
        pub fn set_controller(origin: OriginFor<T>, controller: T::AccountId) -> DispatchResultWithPostInfo {
            let stash = ensure_signed(origin)?;
            // 不允许多个stash指定同一个controller
            ensure!(!<ControllerStash<T>>::contains_key(&controller), Error::<T>::AlreadyController);

            StashController::<T>::insert(stash.clone(), controller.clone());
            ControllerStash::<T>::insert(controller, stash);
            Ok(().into())
        }

        /// stash账户为MachineId设置新的控制账户
        #[pallet::weight(10000)]
        pub fn stash_reset_controller(
            origin: OriginFor<T>,
            machine_id: MachineId,
            new_controller: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let stash = ensure_signed(origin)?;
            let mut machine_info = Self::machines_info(&machine_id);

            let raw_controller = machine_info.controller.clone();
            let mut raw_controller_machines = Self::controller_machines(&raw_controller);
            let mut new_controller_machines = Self::controller_machines(&new_controller);

            ensure!(machine_info.machine_stash == stash, Error::<T>::NotMachineStash);
            machine_info.controller = new_controller.clone();

            // 更改controller_machines
            if let Ok(index) = raw_controller_machines.binary_search(&machine_id) {
                raw_controller_machines.remove(index);

                if let Err(index) = new_controller_machines.binary_search(&machine_id) {
                    new_controller_machines.insert(index, machine_id.clone());
                    ControllerMachines::<T>::insert(&raw_controller, raw_controller_machines);
                    ControllerMachines::<T>::insert(&new_controller, new_controller_machines);
                }
            }

            MachinesInfo::<T>::insert(machine_id, machine_info);

            Ok(().into())
        }

        /// 控制账户重新上架机器，允许修改机器配置，委员会重新审核
        /// NOTE: 用户需要重新添加机器信息，添加机器信息时，检查机器状态，根据掉线时长扣钱
        #[pallet::weight(10000)]
        pub fn reonline_machine(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut live_machines = Self::live_machines();
            let mut machine_info = Self::machines_info(&machine_id);

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);

            // 只允许在线状态的机器修改信息
            match machine_info.machine_status {
                MachineStatus::Online => {},
                _ => return Err(Error::<T>::MachineStatusNotAllowed.into()),
            }

            // 重新上链需要质押一定的手续费
            let stake_amount = T::DbcPrice::get_dbc_amount_by_value(Self::reonline_stake()).unwrap_or_default(); // FIXME

            let stash_stake = Self::stash_stake(&machine_info.machine_stash);
            let new_stash_stake = stash_stake.checked_add(&stake_amount).ok_or(Error::<T>::CalcStakeAmountFailed)?;
            <T as Config>::Currency::set_lock(
                PALLET_LOCK_ID,
                &machine_info.machine_stash,
                new_stash_stake,
                WithdrawReasons::all(),
            );

            UserReonlineStake::<T>::insert(&machine_info.machine_stash, &machine_id, stake_amount);

            Self::change_pos_gpu_by_online(&machine_id, false);
            Self::update_snap_by_online_status(machine_id.clone(), false);

            LiveMachine::rm_machine_id(&mut live_machines.online_machine, &machine_id);
            LiveMachine::add_machine_id(&mut live_machines.bonding_machine, machine_id.clone()); // 放入bonding_machine中

            // NOTE: 记录下机器下线时间，用以在机器重新分派时惩罚用户
            machine_info.machine_status = MachineStatus::StakerReportOffline(now, Box::new(MachineStatus::Online));

            LiveMachines::<T>::put(live_machines);
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Ok(().into())
        }

        /// 控制账户上线一个机器
        /// msg = d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d
        ///     + 5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY
        /// sig 为machine_id对应的私钥对msg进行签名
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

            ensure!(!live_machines.machine_id_exist(&machine_id), Error::<T>::MachineIdExist);
            ensure!(msg.len() == 112, Error::<T>::BadMsgLen); // 验证msg: len(pubkey + account) = 64 + 48

            let sig_machine_id: Vec<u8> = msg[..64].to_vec();
            ensure!(machine_id == sig_machine_id, Error::<T>::SigMachineIdNotEqualBondedMachineId);

            let sig_stash_account: Vec<u8> = msg[64..].to_vec();
            let sig_stash_account =
                Self::get_account_from_str(&sig_stash_account).ok_or(Error::<T>::ConvertMachineIdToWalletFailed)?;
            ensure!(sig_stash_account == stash, Error::<T>::MachineStashNotEqualControllerStash);

            // 验证签名是否为MachineId发出
            if Self::verify_sig(msg.clone(), sig.clone(), machine_id.clone()).is_none() {
                return Err(Error::<T>::BadSignature.into())
            }

            // 用户绑定机器需要质押一张显卡的DBC
            let stake_amount = Self::calc_stake_amount(1).ok_or(Error::<T>::CalcStakeAmountFailed)?;

            // 扣除10个Dbc作为交易手续费
            <generic_func::Module<T>>::pay_fixed_tx_fee(controller.clone()).map_err(|_| Error::<T>::PayTxFeeFailed)?;

            let mut stash_machines = Self::stash_machines(&stash);
            if let Err(index) = stash_machines.total_machine.binary_search(&machine_id) {
                stash_machines.total_machine.insert(index, machine_id.clone());
            }

            let mut controller_machines = Self::controller_machines(&controller);
            if let Err(index) = controller_machines.binary_search(&machine_id) {
                controller_machines.insert(index, machine_id.clone());
            }

            // 添加到LiveMachine的bonding_machine字段
            LiveMachine::add_machine_id(&mut live_machines.bonding_machine, machine_id.clone());

            // 初始化MachineInfo, 并添加到MachinesInfo
            let machine_info = MachineInfo {
                controller: controller.clone(),
                machine_stash: stash.clone(),
                bonding_height: <frame_system::Module<T>>::block_number(),
                stake_amount,
                machine_status: MachineStatus::AddingCustomizeInfo,
                ..Default::default()
            };

            Self::add_user_total_stake(&stash, stake_amount).map_err(|_| Error::<T>::BalanceNotEnough)?;

            ControllerMachines::<T>::insert(&controller, controller_machines);
            StashMachines::<T>::insert(&stash, stash_machines);
            LiveMachines::<T>::put(live_machines);
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::BondMachine(controller.clone(), machine_id.clone(), stake_amount));
            Ok(().into())
        }

        /// 控制账户生成机房信息,该机房记录到stash账户下
        #[pallet::weight(10000)]
        pub fn gen_server_room(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashBond)?;

            // 扣除10个Dbc作为交易手续费
            <generic_func::Module<T>>::pay_fixed_tx_fee(controller.clone()).map_err(|_| Error::<T>::PayTxFeeFailed)?;

            let new_server_room = <generic_func::Module<T>>::random_server_room();

            let mut stash_server_rooms = Self::stash_server_rooms(&stash);
            if let Err(index) = stash_server_rooms.binary_search(&new_server_room) {
                stash_server_rooms.insert(index, new_server_room);
            }

            StashServerRooms::<T>::insert(&stash, stash_server_rooms);
            Ok(().into())
        }

        /// 控制账户添加机器信息: 经纬度*10^4取整
        #[pallet::weight(10000)]
        pub fn add_machine_info(
            origin: OriginFor<T>,
            machine_id: MachineId,
            customize_machine_info: StakerCustomizeInfo,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            if customize_machine_info.telecom_operators.len() == 0 {
                return Err(Error::<T>::TelecomIsNull.into())
            }

            // 查询机器Id是否在该账户的控制下
            let mut machine_info = Self::machines_info(&machine_id);
            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);

            let stash_server_rooms = Self::stash_server_rooms(&machine_info.machine_stash);
            if stash_server_rooms.binary_search(&customize_machine_info.server_room).is_err() {
                return Err(Error::<T>::ServerRoomNotFound.into())
            }

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
            if let Ok(index) = live_machines.bonding_machine.binary_search(&machine_id) {
                live_machines.bonding_machine.remove(index);
                if let Err(index) = live_machines.confirmed_machine.binary_search(&machine_id) {
                    live_machines.confirmed_machine.insert(index, machine_id.clone());
                }
                LiveMachines::<T>::put(live_machines);

                // 当机器下线，并且在bonding_machine中时，表明该机器正在重新上线
                // TODO: 按照下线时间惩罚
                if let MachineStatus::StakerReportOffline(..) = machine_info.machine_status {}

                machine_info.machine_status = MachineStatus::DistributingOrder;
            }
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Ok(().into())
        }

        /// 机器处于补交质押状态时，需要补交质押才能上线
        #[pallet::weight(10000)]
        pub fn fulfill_machine(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let current_era = Self::current_era();

            let mut machine_info = Self::machines_info(&machine_id);
            let mut live_machine = Self::live_machines();

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);

            let stake_need = Self::calc_stake_amount(machine_info.machine_info_detail.committee_upload_info.gpu_num)
                .ok_or(Error::<T>::CalcStakeAmountFailed)?;

            if machine_info.stake_amount < stake_need {
                let extra_stake = stake_need - machine_info.stake_amount;

                Self::add_user_total_stake(&machine_info.machine_stash, extra_stake)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;

                machine_info.stake_amount = stake_need;
            }
            machine_info.machine_status = MachineStatus::Online;
            machine_info.online_height = now;
            machine_info.last_online_height = now;
            machine_info.reward_deadline = current_era + REWARD_DURATION;
            machine_info.last_machine_restake = now;

            // TODO: 将这个改到update_snap_by_online_status中
            Self::change_pos_gpu_by_online(&machine_id, true);
            Self::update_snap_by_online_status(machine_id.clone(), true);

            LiveMachine::rm_machine_id(&mut live_machine.fulfilling_machine, &machine_id);
            LiveMachine::add_machine_id(&mut live_machine.online_machine, machine_id.clone());

            LiveMachines::<T>::put(live_machine);
            MachinesInfo::<T>::insert(&machine_id, machine_info);
            Ok(().into())
        }

        /// 如果绑定失败，会扣除5%的DBC（5000），要在10天内手动执行rebond并补充质押
        #[pallet::weight(10000)]
        pub fn rebond_machine(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            // let now = <frame_system::Module<T>>::block_number();
            let mut machine_info = Self::machines_info(&machine_id);

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);
            match machine_info.machine_status {
                MachineStatus::CommitteeRefused(_) => {},
                _ => return Err(Error::<T>::NotRefusedMachine.into()),
            }

            // 补充质押
            let stake_need = Self::stake_per_gpu();
            if stake_need > machine_info.stake_amount {
                let extra_stake = stake_need - machine_info.stake_amount;

                Self::add_user_total_stake(&machine_info.machine_stash, extra_stake)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;

                machine_info.stake_amount = stake_need;
            }

            let mut live_machines = Self::live_machines();
            LiveMachine::rm_machine_id(&mut live_machines.refused_machine, &machine_id);
            LiveMachine::add_machine_id(&mut live_machines.confirmed_machine, machine_id.clone());

            machine_info.machine_status = MachineStatus::DistributingOrder;

            MachinesInfo::<T>::insert(&machine_id, machine_info);
            LiveMachines::<T>::put(live_machines);

            Ok(().into())
        }

        /// 控制账户进行领取收益到stash账户
        #[pallet::weight(10000)]
        pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash_account = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashAccount)?;
            ensure!(StashMachines::<T>::contains_key(&stash_account), Error::<T>::NotMachineController);
            let mut stash_machine = Self::stash_machines(&stash_account);

            <T as pallet::Config>::Currency::deposit_into_existing(&stash_account, stash_machine.can_claim_reward)
                .map_err(|_| Error::<T>::ClaimRewardFailed)?;

            stash_machine.total_claimed_reward += stash_machine.can_claim_reward;
            stash_machine.can_claim_reward = 0u64.saturated_into();
            StashMachines::<T>::insert(&stash_account, stash_machine);

            Ok(().into())
        }

        /// 控制账户报告机器下线:Online/Rented时允许
        #[pallet::weight(10000)]
        pub fn controller_report_offline(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            let machine_info = Self::machines_info(&machine_id);
            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);

            debug::RuntimeLogger::init();

            // 某些状态允许下线
            match machine_info.machine_status {
                MachineStatus::Online | MachineStatus::Rented => {},
                _ => {
                    debug::error!("Machine Status::{:?}", machine_info.machine_status);
                    return Err(Error::<T>::MachineStatusNotAllowed.into())
                },
            }

            Self::machine_offline(machine_id);
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
                machine_info.last_online_height.saturated_into::<u64>() > BLOCK_PER_ERA,
                Error::<T>::CannotOnlineTwiceOneDay
            );

            // MachineStatus改为之前的状态
            if let MachineStatus::StakerReportOffline(offline_time, status) = machine_info.machine_status {
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

                        if let Err(index) = stash_machine.online_machine.binary_search(&machine_id) {
                            stash_machine.online_machine.insert(index, machine_id.clone());
                        }
                        stash_machine.total_gpu_num += gpu_num;
                    },
                    _ => {},
                }
                match machine_info.machine_status {
                    MachineStatus::Rented => {
                        sys_info.total_rented_gpu += gpu_num;
                        // machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;
                        Self::update_snap_by_rent_status(machine_id.clone(), true);
                        Self::change_pos_gpu_by_rent(&machine_id, true);
                        stash_machine.total_rented_gpu += gpu_num;

                        // 机器在被租用状态下线，会被惩罚
                        Self::slash_when_report_offline(
                            machine_id.clone(),
                            StashSlashReason::RentedReportOffline(offline_duration),
                            None,
                            None,
                        );
                    },
                    MachineStatus::Online => {
                        Self::slash_when_report_offline(
                            machine_id.clone(),
                            StashSlashReason::OnlineReportOffline(offline_duration),
                            None,
                            None,
                        );
                    },
                    _ => {},
                }
            } else {
                return Err(Error::<T>::MachineStatusNotAllowed.into())
            }

            machine_info.last_online_height = now;

            LiveMachine::rm_machine_id(&mut live_machine.offline_machine, &machine_id);
            match machine_info.machine_status {
                MachineStatus::WaitingFulfill => {
                    LiveMachine::add_machine_id(&mut live_machine.fulfilling_machine, machine_id.clone());
                },
                _ => {
                    LiveMachine::add_machine_id(&mut live_machine.online_machine, machine_id.clone());
                },
            }

            LiveMachines::<T>::put(live_machine);
            StashMachines::<T>::insert(&machine_info.machine_stash, stash_machine);
            MachinesInfo::<T>::insert(&machine_id, machine_info);
            SysInfo::<T>::put(sys_info);

            Ok(().into())
        }

        /// 超过365天的机器可以在距离上次租用10天，且没被租用时退出
        #[pallet::weight(10000)]
        pub fn claim_exit(origin: OriginFor<T>, _controller: T::AccountId) -> DispatchResultWithPostInfo {
            let _controller = ensure_signed(origin)?;
            Ok(().into())
        }

        /// 满足365天可以申请重新质押，退回质押币
        /// 在系统中上线满365天之后，可以按当时机器需要的质押数量，重新入网。多余的币解绑
        /// 在重新上线之后，下次再执行本操作，需要等待365天
        #[pallet::weight(10000)]
        pub fn rebond_online_machine(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let one_year = 1051200u32; // 365 * 2880
            let mut machine_info = Self::machines_info(&machine_id);

            ensure!(controller == machine_info.controller, Error::<T>::NotMachineController);
            ensure!(now - machine_info.last_machine_restake >= one_year.into(), Error::<T>::TooFastToReStake);

            // 计算现在需要多少质押
            let stake_need = Self::calc_stake_amount(machine_info.machine_info_detail.committee_upload_info.gpu_num)
                .ok_or(Error::<T>::CalcStakeAmountFailed)?;
            ensure!(machine_info.stake_amount > stake_need, Error::<T>::NoStakeToReduce);

            if let Some(extra_stake) = machine_info.stake_amount.checked_sub(&stake_need) {
                machine_info.stake_amount = stake_need;
                machine_info.last_machine_restake = now;
                if Self::reduce_user_total_stake(&machine_info.machine_stash, extra_stake).is_err() {
                    return Err(Error::<T>::ReduceStakeFailed.into())
                }
                MachinesInfo::<T>::insert(&machine_id, machine_info);
            }

            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        BondMachine(T::AccountId, MachineId, BalanceOf<T>),
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
    }
}

impl<T: Config> Pallet<T> {
    /// 更改委员会质押状态
    // fn change_stash_used_stake(&stash_account: T::AccountId, amount: BalanceOf<T>, is_add: bool) -> Result<(), ()> {
    //     let mut stash_stake = Self::stash_machine_stake(&stash_account);
    //     Ok(())
    // }

    fn slash_when_report_offline(
        machine_id: MachineId,
        slash_reason: StashSlashReason<T::BlockNumber>,
        reporter: Option<T::AccountId>,
        committee: Option<Vec<T::AccountId>>,
    ) {
        let machine_info = Self::machines_info(&machine_id);
        match slash_reason {
            // 算工主动报告被租用的机器，主动下线
            StashSlashReason::RentedReportOffline(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return,
                    // 下线不超过7分钟
                    1..=14 => {
                        // 扣除2%质押币。100%进入国库。
                        Self::do_slash_deposit(2, machine_id, None, None);
                    },
                    // 不超过48小时
                    15..=5760 => {
                        // 扣除4%质押币。100%进入国库
                        Self::do_slash_deposit(4, machine_id, None, None);
                    },
                    // 不超过120小时
                    5761..=14400 => {
                        // 扣除30%质押币，10%给到用户，90%进入国库
                        Self::do_slash_deposit(30, machine_id, machine_info.last_machine_renter, None);
                    },
                    // 超过120小时
                    _ => {
                        // 扣除50%押金。10%给到用户，90%进入国库
                        Self::do_slash_deposit(50, machine_id, machine_info.last_machine_renter, None);
                    },
                }
            },
            // 算工主动报告在线的机器，主动下线
            StashSlashReason::OnlineReportOffline(duration) => {
                // TODO: 判断是否已经下线十天，如果是，则不进行惩罚，仅仅下线处理
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return,
                    // 下线不超过7分钟
                    1..=14 => {
                        // 扣除2%质押币，质押币全部进入国库。
                        Self::do_slash_deposit(2, machine_id, None, None);
                    },
                    // 下线不超过48小时
                    15..=5760 => {
                        // 扣除4%质押币，质押币全部进入国库
                        Self::do_slash_deposit(4, machine_id, None, None);
                    },
                    // 不超过240小时
                    5761..=28800 => {
                        // 扣除30%质押币，质押币全部进入国库
                        Self::do_slash_deposit(30, machine_id, None, None);
                    },
                    _ => {
                        // TODO: 如果机器从首次上线时间起超过365天，剩下20%押金可以申请退回。

                        // 扣除80%质押币。质押币全部进入国库。
                        Self::do_slash_deposit(80, machine_id, None, None);
                    },
                }
            },
            // 机器处于租用状态，无法访问
            StashSlashReason::RentedInaccessible(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return,
                    // 不超过7分钟
                    1..=14 => {
                        // 扣除4%质押币。10%给验证人，90%进入国库
                        Self::do_slash_deposit(4, machine_id, reporter, None);
                    },
                    // 不超过48小时
                    15..=5760 => {
                        // 扣除8%质押币。10%给验证人，90%进入国库
                        Self::do_slash_deposit(8, machine_id, reporter, None);
                    },
                    // 不超过120小时
                    5761..=14400 => {
                        // 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(60, machine_id, reporter, committee);
                    },
                    // 超过120小时
                    _ => {
                        // 扣除100%押金。10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(100, machine_id, reporter, committee);
                    },
                }
            },
            // 机器处于租用状态，机器出现故障
            StashSlashReason::RentedHardwareMalfunction(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return,
                    //不超过4小时
                    1..=480 => {
                        // 扣除6%质押币。10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(6, machine_id, reporter, committee);
                    },
                    // 不超过24小时
                    481..=2880 => {
                        // 扣除12%质押币。10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(12, machine_id, reporter, committee);
                    },
                    // 不超过48小时
                    2881..=5760 => {
                        // 扣除16%质押币。10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(16, machine_id, reporter, committee);
                    },
                    // 不超过120小时
                    5761..=14400 => {
                        // 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(60, machine_id, reporter, committee);
                    },
                    _ => {
                        // 扣除100%押金，10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(100, machine_id, reporter, committee);
                    },
                }
            },
            // 机器处于租用状态，机器硬件造假
            StashSlashReason::RentedHardwareCounterfeit(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return,
                    // 下线不超过4小时
                    1..=480 => {
                        // 扣除12%质押币。10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(12, machine_id, reporter, committee);
                    },
                    // 不超过24小时
                    481..=2880 => {
                        // 扣除24%质押币。10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(12, machine_id, reporter, committee);
                    },
                    // 不超过48小时
                    2881..=5760 => {
                        // 扣除32%质押币。10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(12, machine_id, reporter, committee);
                    },
                    // 不超过120小时
                    5761..=14400 => {
                        // 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(12, machine_id, reporter, committee);
                    },
                    _ => {
                        // 扣除100%押金，10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(12, machine_id, reporter, committee);
                    },
                }
            },
            // 机器在线，被举报无法租用
            StashSlashReason::OnlineRentFailed(duration) => {
                let duration = duration.saturated_into::<u64>();
                match duration {
                    0 => return,
                    1..=480 => {
                        // 扣除6%质押币。10%给到用户，20%给到验证人，50%进入国库
                        Self::do_slash_deposit(12, machine_id, reporter, committee);
                    },
                    481..=2880 => {
                        // 扣除12%质押币。10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(12, machine_id, reporter, committee);
                    },
                    2881..=5760 => {
                        // 扣除16%质押币。10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(12, machine_id, reporter, committee);
                    },
                    5761..=14400 => {
                        // 扣除60%质押币。10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(12, machine_id, reporter, committee);
                    },
                    _ => {
                        // 扣除100%押金，10%给到用户，20%给到验证人，70%进入国库
                        Self::do_slash_deposit(12, machine_id, reporter, committee);
                    },
                }
            },
        }
    }

    // 惩罚掉机器押金，如果执行惩罚后机器押金不够，则状态变为补充质押
    fn do_slash_deposit(
        slash_deposit_percent: u64,
        machine_id: MachineId,
        reporter: Option<T::AccountId>,
        committee: Option<Vec<T::AccountId>>,
    ) {
        let slash_percent = Perbill::from_rational_approximation(slash_deposit_percent, 100);
        let machine_info = Self::machines_info(&machine_id);
        // 当前机器需要的质押数量
        let stake_need =
            Self::calc_stake_amount(machine_info.machine_info_detail.committee_upload_info.gpu_num).unwrap(); // TODO: handle error

        // 惩罚的质押数量
        let slash_amount = slash_percent * machine_info.stake_amount;

        // 添加惩罚
        <T as pallet::Config>::ManageCommittee::add_slash(machine_info.machine_stash.clone(), slash_amount, vec![]);

        // 机器质押不够时，将机器下线，并需要补充质押
        if stake_need > machine_info.stake_amount {
            // 将机器下线，执行下线逻辑
            Self::machine_offline(machine_id.clone());

            let mut machine_info = Self::machines_info(&machine_id);
            machine_info.stake_amount -= slash_amount;
            machine_info.machine_status = MachineStatus::WaitingFulfill;
            MachinesInfo::<T>::insert(&machine_id, machine_info);
        }
    }

    // For upgrade
    fn get_all_machine_id() -> Vec<MachineId> {
        <MachinesInfo<T> as IterableStorageMap<MachineId, _>>::iter()
            .map(|(machine_id, _)| machine_id)
            .collect::<Vec<_>>()
    }

    /// 委员会确定机器重新上架
    fn lc_confirm_machine_reonline(
        reported_committee: Vec<T::AccountId>,
        committee_upload_info: CommitteeUploadInfo,
    ) -> Result<(), ()> {
        let mut machine_info = Self::machines_info(&committee_upload_info.machine_id);
        let mut live_machines = Self::live_machines();
        let now = <frame_system::Module<T>>::block_number();

        LiveMachine::rm_machine_id(&mut live_machines.booked_machine, &committee_upload_info.machine_id);

        machine_info.machine_info_detail.committee_upload_info = committee_upload_info.clone();

        let stake_need = Self::calc_stake_amount(committee_upload_info.gpu_num).ok_or(())?;
        if let Some(extra_stake) = stake_need.checked_sub(&machine_info.stake_amount) {
            if Self::add_user_total_stake(&machine_info.machine_stash, extra_stake).is_ok() {
                LiveMachine::add_machine_id(
                    &mut live_machines.online_machine,
                    committee_upload_info.machine_id.clone(),
                );
                machine_info.stake_amount = stake_need;
                machine_info.machine_status = MachineStatus::Online;
                machine_info.last_machine_restake = now;
                machine_info.last_online_height = now;
            } else {
                LiveMachine::add_machine_id(
                    &mut live_machines.fulfilling_machine,
                    committee_upload_info.machine_id.clone(),
                );
                machine_info.machine_status = MachineStatus::WaitingFulfill;
            }
        } else {
            LiveMachine::add_machine_id(&mut live_machines.online_machine, committee_upload_info.machine_id.clone());
            machine_info.machine_status = MachineStatus::Online;
        }

        // 根据质押，奖励给这些委员会
        let reonline_stake = Self::user_reonline_stake(&machine_info.machine_stash, &committee_upload_info.machine_id);
        let reward_each_get = Perbill::from_rational_approximation(1, reported_committee.len() as u64) * reonline_stake;
        if Self::reduce_user_total_stake(&machine_info.machine_stash, reonline_stake).is_ok() {
            for a_committee in reported_committee {
                if <T as pallet::Config>::Currency::transfer(
                    &machine_info.machine_stash,
                    &a_committee,
                    reward_each_get,
                    KeepAlive,
                )
                .is_err()
                {
                    // TODO: handler error here
                };
            }

            UserReonlineStake::<T>::remove(&machine_info.machine_stash, &committee_upload_info.machine_id);
        } // TODO: handle failed case

        MachinesInfo::<T>::insert(committee_upload_info.machine_id.clone(), machine_info.clone());
        LiveMachines::<T>::put(live_machines);

        // NOTE: Must be after MachinesInfo change, which depend on machine_info
        if let MachineStatus::Online = machine_info.machine_status {
            Self::change_pos_gpu_by_online(&committee_upload_info.machine_id, true);
            Self::update_snap_by_online_status(committee_upload_info.machine_id, true);
        }

        Ok(())
    }

    // TODO
    fn lc_refuse_machine_reonline(_machine_id: MachineId) {}

    /// 下架机器
    fn machine_offline(machine_id: MachineId) {
        let mut machine_info = Self::machines_info(&machine_id);
        let mut live_machine = Self::live_machines();

        let now = <frame_system::Module<T>>::block_number();

        if let MachineStatus::Rented = machine_info.machine_status {
            Self::update_snap_by_rent_status(machine_id.clone(), false);
            Self::change_pos_gpu_by_rent(&machine_id, false);
        }

        // When offline, pos_info will be removed
        Self::change_server_room_gpu(&machine_id, false);
        Self::change_pos_gpu_by_online(&machine_id, false);
        Self::update_snap_by_online_status(machine_id.clone(), false);

        LiveMachine::rm_machine_id(&mut live_machine.online_machine, &machine_id);
        LiveMachine::add_machine_id(&mut live_machine.offline_machine, machine_id.clone());

        // After re-online, machine status is same as former
        machine_info.machine_status = MachineStatus::StakerReportOffline(now, Box::new(machine_info.machine_status));

        LiveMachines::<T>::put(live_machine);
        MachinesInfo::<T>::insert(&machine_id, machine_info);
    }

    /// 特定机房机器上线/下线
    fn change_server_room_gpu(machine_id: &MachineId, is_online: bool) {
        let machine_info = Self::machines_info(machine_id);
        let server_room = machine_info.machine_info_detail.staker_customize_info.server_room;
        let mut server_room_machines = Self::server_room_machines(server_room).unwrap_or_default();

        if is_online {
            if let Err(index) = server_room_machines.binary_search(machine_id) {
                server_room_machines.insert(index, machine_id.to_vec());
            }
        } else {
            if let Ok(index) = server_room_machines.binary_search(machine_id) {
                server_room_machines.remove(index);
            }
        }

        ServerRoomMachines::<T>::insert(server_room, server_room_machines);
    }

    /// 特定位置GPU上线/下线
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

    // FIXME: 检查该逻辑
    // 检查fulfilling list，如果超过10天，则清除记录，退还质押
    fn clean_refused_machine() {
        let mut live_machines = Self::live_machines();
        if live_machines.refused_machine.len() == 0 {
            return
        }

        let mut sys_info = Self::sys_info();

        let mut live_machines_is_changed = false;
        let now = <frame_system::Module<T>>::block_number();

        let refused_machine = live_machines.refused_machine.clone();

        for a_machine in refused_machine {
            let machine_info = Self::machines_info(&a_machine);
            match machine_info.machine_status {
                MachineStatus::CommitteeRefused(refuse_time) => {
                    if now - refuse_time > (10 * 2880u32).saturated_into::<T::BlockNumber>() {
                        LiveMachine::rm_machine_id(&mut live_machines.refused_machine, &a_machine);

                        live_machines_is_changed = true;

                        if T::ManageCommittee::change_used_stake.is_err(
                            &machine_info.machine_stash,
                            machine_info.stake_amount,
                            false,
                        ) {
                            debug::error!("Reduce user stake failed");
                            continue
                        }
                        if let Some(value) = sys_info.total_stake.checked_sub(&machine_info.stake_amount) {
                            sys_info.total_stake = value;
                        } else {
                            debug::error!("Reduce total stake failed");
                            continue
                        }

                        let mut controller_machines = Self::controller_machines(&machine_info.controller);
                        if let Ok(index) = controller_machines.binary_search(&a_machine) {
                            controller_machines.remove(index);
                        }

                        let mut stash_machines = Self::stash_machines(&machine_info.machine_stash);
                        if let Ok(index) = stash_machines.total_machine.binary_search(&a_machine) {
                            stash_machines.total_machine.remove(index);
                        }

                        ControllerMachines::<T>::insert(&machine_info.controller, controller_machines);
                        StashMachines::<T>::insert(&machine_info.machine_stash, stash_machines);
                        MachinesInfo::<T>::remove(a_machine);
                    }
                },
                _ => {},
            }
        }
        if live_machines_is_changed {
            LiveMachines::<T>::put(live_machines);
            SysInfo::<T>::put(sys_info);
        }
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
    // (10000, +) -> min( 100000 * 10000 / (10000 + n), 5w RMB DBC )
    pub fn calc_stake_amount(gpu_num: u32) -> Option<BalanceOf<T>> {
        let sys_info = Self::sys_info();

        let mut base_stake = Self::stake_per_gpu(); // 单卡10_0000 DBC
        if sys_info.total_gpu_num > 10_000 {
            base_stake = Perbill::from_rational_approximation(10_000u64, sys_info.total_gpu_num) * base_stake
        }

        let stake_usd_limit = Self::stake_usd_limit();
        let stake_limit = T::DbcPrice::get_dbc_amount_by_value(stake_usd_limit)?;

        return base_stake.min(stake_limit).checked_mul(&gpu_num.saturated_into::<BalanceOf<T>>())
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

    // TODO: 正式网将修改奖励时间
    /// 计算当前Era在线奖励数量
    fn current_era_reward() -> Option<BalanceOf<T>> {
        let current_era = Self::current_era() as u64;
        let reward_start_era = Self::reward_start_era()? as u64;

        if current_era < reward_start_era {
            return None
        }

        let era_duration = current_era - reward_start_era;

        let phase_index = if era_duration < 30 {
            0
        } else if era_duration < 30 + 730 {
            1
        } else if era_duration < 30 + 730 + 270 {
            2
        } else if era_duration < 30 + 730 + 270 + 1825 {
            3
        } else {
            4
        };

        Self::phase_n_reward_per_era(phase_index)
    }

    fn add_user_total_stake(who: &T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        let stash_stake = Self::stash_stake(who);
        let new_stash_stake = stash_stake.checked_add(&amount).ok_or(())?;

        // 改变总质押
        let mut sys_info = Self::sys_info();
        sys_info.total_stake = sys_info.total_stake.checked_add(&amount).ok_or(())?;

        <T as Config>::Currency::set_lock(PALLET_LOCK_ID, who, new_stash_stake, WithdrawReasons::all());

        StashStake::<T>::insert(who, new_stash_stake);
        SysInfo::<T>::put(sys_info);
        Ok(())
    }

    fn reduce_user_total_stake(who: &T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        let stash_stake = Self::stash_stake(who);
        let new_stash_stake = stash_stake.checked_sub(&amount).ok_or(())?;

        // 改变总质押
        let mut sys_info = Self::sys_info();
        sys_info.total_stake = sys_info.total_stake.checked_sub(&amount).ok_or(())?;

        <T as Config>::Currency::set_lock(PALLET_LOCK_ID, who, new_stash_stake, WithdrawReasons::all());
        StashStake::<T>::insert(&who, new_stash_stake);
        SysInfo::<T>::put(sys_info);

        Ok(())
    }

    // 获取下一Era stash grade即为当前Era stash grade
    fn get_stash_grades(era_index: EraIndex, stash: &T::AccountId) -> u64 {
        let next_era_stash_snapshot = Self::eras_stash_points(era_index).unwrap_or_default();

        if let Some(stash_snapshot) = next_era_stash_snapshot.staker_statistic.get(stash) {
            return stash_snapshot.total_grades().unwrap()
        }
        0
    }

    // 当机器Online状态变化时，更改系统存储
    // Online:
    // - Reads:
    // - Writes:
    // Offline:
    // - Reads:
    // - Writes:
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
                    reward_account: machine_info.reward_committee,
                },
            );

            if let Err(index) = stash_machine.online_machine.binary_search(&machine_id) {
                stash_machine.online_machine.insert(index, machine_id.clone());
            }
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

            if let Ok(index) = stash_machine.online_machine.binary_search(&machine_id) {
                stash_machine.online_machine.remove(index);
            }

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

    fn update_snap_by_rent_status(machine_id: MachineId, is_rented: bool) {
        let machine_info = Self::machines_info(&machine_id);
        let current_era = Self::current_era();

        let mut current_era_stash_snap = Self::eras_stash_points(current_era).unwrap();
        let mut next_era_stash_snap = Self::eras_stash_points(current_era + 1).unwrap();
        let mut current_era_machine_snap = Self::eras_machine_points(current_era).unwrap();
        let mut next_era_machine_snap = Self::eras_machine_points(current_era + 1).unwrap();

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
                reward_account: machine_info.reward_committee.clone(),
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
                    reward_account: machine_info.reward_committee.clone(),
                },
            );
        }

        // 被租用或者退租都影响下一Era记录，而退租直接影响当前得分
        ErasStashPoints::<T>::insert(current_era + 1, next_era_stash_snap);
        ErasMachinePoints::<T>::insert(current_era + 1, next_era_machine_snap);
        if !is_rented {
            ErasStashPoints::<T>::insert(current_era, current_era_stash_snap);
            ErasMachinePoints::<T>::insert(current_era, current_era_machine_snap);
        }

        let new_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);
        stash_machine.total_calc_points = stash_machine.total_calc_points + new_stash_grade - old_stash_grade;
        sys_info.total_calc_points = sys_info.total_calc_points + new_stash_grade - old_stash_grade;

        SysInfo::<T>::put(sys_info);
        StashMachines::<T>::insert(&machine_info.machine_stash, stash_machine);
    }

    // 根据机器得分快照，和委员会膨胀分数，计算应该奖励
    // end_era分发奖励
    fn distribute_reward() -> Result<(), ()> {
        let current_era = Self::current_era();
        let start_era = if current_era > 150 { current_era - 150 } else { 0u32 };
        let all_stash = Self::get_all_stash();

        // 释放75%的奖励
        for era_index in start_era..=current_era {
            let era_machine_points = Self::eras_machine_points(era_index).unwrap();
            let era_stash_points = Self::eras_stash_points(era_index).unwrap();

            // update era_reward
            let era_reward = Self::era_reward(era_index);

            for a_stash in &all_stash {
                let mut stash_machine = Self::stash_machines(a_stash);

                for machine_id in stash_machine.total_machine.clone() {
                    let machine_info = Self::machines_info(&machine_id);

                    // 计算当时机器实际获得的奖励
                    let machine_points = era_machine_points.get(&machine_id);
                    let stash_points = era_stash_points.staker_statistic.get(&machine_info.machine_stash);

                    if machine_points.is_none() || stash_points.is_none() {
                        continue
                    }
                    let machine_points = machine_points.unwrap();
                    let stash_points = stash_points.unwrap();

                    let machine_actual_grade = machine_points.machine_actual_grade(stash_points.inflation);

                    // 该Era机器获得的总奖励
                    let machine_total_reward = Perbill::from_rational_approximation(
                        machine_actual_grade as u64,
                        era_stash_points.total as u64,
                    ) * era_reward;

                    let linear_reward_part = Perbill::from_rational_approximation(75u64, 100u64) * machine_total_reward;

                    let release_now = if era_index == current_era {
                        // FIXME: 考虑1%给委员会的部分
                        ErasMachineReward::<T>::insert(current_era, &machine_id, machine_total_reward);
                        ErasStashReward::<T>::mutate(&current_era, &machine_info.machine_stash, |old_value| {
                            *old_value + machine_total_reward
                        });

                        machine_total_reward - linear_reward_part
                    } else {
                        // 剩余75%的1/150
                        Perbill::from_rational_approximation(1u32, 150u32) * linear_reward_part
                    };

                    if machine_points.reward_account.len() == 0 || current_era >= machine_info.reward_deadline {
                        // 没有委员会来分，则全部奖励给stash账户
                        stash_machine.can_claim_reward += release_now;
                        if era_index == current_era {
                            stash_machine.total_earned_reward += machine_total_reward;
                        }

                        ErasMachineReleasedReward::<T>::mutate(&current_era, &machine_id, |old_value| {
                            *old_value + release_now
                        });
                        ErasStashReleasedReward::<T>::mutate(&current_era, &machine_info.machine_stash, |old_value| {
                            *old_value + release_now
                        });
                    } else {
                        if era_index == current_era {
                            // 修复：如果委员的奖励时间会很快就要结束了
                            // 则奖励的前一部分给委员会一部分，后一部分，不给委员会

                            if machine_info.reward_deadline - current_era >= 150 {
                                stash_machine.total_earned_reward = stash_machine.total_earned_reward +
                                    Perbill::from_rational_approximation(99u64, 100u64) * machine_total_reward;
                            } else if current_era > machine_info.reward_deadline {
                                stash_machine.total_earned_reward =
                                    stash_machine.total_earned_reward + machine_total_reward;
                            } else {
                                // reward_to_committee:
                                let reward_to_committee = machine_info.reward_deadline - current_era;

                                let total_reward_before_deadline =
                                    Perbill::from_rational_approximation(reward_to_committee, 150) *
                                        machine_total_reward;
                                let total_reward_after_deadline = machine_total_reward - total_reward_before_deadline;

                                let reward_to_stash_before_deadline =
                                    Perbill::from_rational_approximation(99u32, 100u32) * total_reward_before_deadline;

                                stash_machine.total_earned_reward = stash_machine.total_earned_reward +
                                    total_reward_after_deadline +
                                    reward_to_stash_before_deadline;
                            }
                        }

                        // 99% 分给stash账户
                        let release_to_stash = Perbill::from_rational_approximation(99u64, 100u64) * release_now;
                        stash_machine.can_claim_reward += release_to_stash;

                        ErasMachineReleasedReward::<T>::mutate(&current_era, &machine_id, |old_value| {
                            *old_value + release_to_stash
                        });
                        ErasStashReleasedReward::<T>::mutate(&current_era, &machine_info.machine_stash, |old_value| {
                            *old_value + release_now
                        });

                        // 剩下分给committee
                        let release_to_committee = release_now - release_to_stash;
                        let committee_each_get =
                            Perbill::from_rational_approximation(1u64, machine_points.reward_account.len() as u64) *
                                release_to_committee;

                        for a_committee in machine_points.reward_account.clone() {
                            T::ManageCommittee::add_reward(a_committee, committee_each_get);
                        }
                    }
                }

                StashMachines::<T>::insert(a_stash, stash_machine);
            }
        }

        Ok(())
    }
}

/// 审查委员会可以执行的操作
impl<T: Config> LCOps for Pallet<T> {
    type MachineId = MachineId;
    type AccountId = T::AccountId;
    type CommitteeUploadInfo = CommitteeUploadInfo;

    // 委员会订阅了一个机器ID
    // 将机器状态从ocw_confirmed_machine改为booked_machine，同时将机器状态改为booked
    fn lc_booked_machine(id: MachineId) {
        let mut live_machines = Self::live_machines();

        LiveMachine::rm_machine_id(&mut live_machines.confirmed_machine, &id);
        LiveMachine::add_machine_id(&mut live_machines.booked_machine, id.clone());

        let mut machine_info = Self::machines_info(&id);
        machine_info.machine_status = MachineStatus::CommitteeVerifying;

        LiveMachines::<T>::put(live_machines);
        MachinesInfo::<T>::insert(&id, machine_info);
    }

    /// 由于委员会没有达成一致，需要重新返回到bonding_machine
    fn lc_revert_booked_machine(id: MachineId) {
        let mut live_machines = Self::live_machines();

        LiveMachine::rm_machine_id(&mut live_machines.booked_machine, &id);
        LiveMachine::add_machine_id(&mut live_machines.confirmed_machine, id.clone());

        let mut machine_info = Self::machines_info(&id);
        machine_info.machine_status = MachineStatus::DistributingOrder;

        LiveMachines::<T>::put(live_machines);
        MachinesInfo::<T>::insert(&id, machine_info);
    }

    // 当多个委员会都对机器进行了确认之后，添加机器信息，并更新机器得分
    // 机器被成功添加, 则添加上可以获取收益的委员会
    fn lc_confirm_machine(
        reported_committee: Vec<T::AccountId>,
        committee_upload_info: CommitteeUploadInfo,
    ) -> Result<(), ()> {
        let now = <frame_system::Module<T>>::block_number();
        let current_era = Self::current_era();

        let mut machine_info = Self::machines_info(&committee_upload_info.machine_id);
        let mut live_machines = Self::live_machines();

        if let MachineStatus::StakerReportOffline(..) = machine_info.machine_status {
            Self::lc_confirm_machine_reonline(reported_committee, committee_upload_info)?;
            return Ok(())
        }

        LiveMachine::rm_machine_id(&mut live_machines.booked_machine, &committee_upload_info.machine_id);

        machine_info.machine_info_detail.committee_upload_info = committee_upload_info.clone();
        machine_info.reward_committee = reported_committee;

        // 改变用户的绑定数量。如果用户余额足够，则直接质押。否则将机器状态改为补充质押
        let stake_need = Self::calc_stake_amount(committee_upload_info.gpu_num).ok_or(())?;
        if let Some(extra_stake) = stake_need.checked_sub(&machine_info.stake_amount) {
            if Self::add_user_total_stake(&machine_info.machine_stash, extra_stake).is_ok() {
                LiveMachine::add_machine_id(
                    &mut live_machines.online_machine,
                    committee_upload_info.machine_id.clone(),
                );
                machine_info.stake_amount = stake_need;
                machine_info.machine_status = MachineStatus::Online;
                machine_info.online_height = now;
                machine_info.last_online_height = now;
                machine_info.reward_deadline = current_era + REWARD_DURATION;
                machine_info.last_machine_restake = now;
            } else {
                LiveMachine::add_machine_id(
                    &mut live_machines.fulfilling_machine,
                    committee_upload_info.machine_id.clone(),
                );
                machine_info.machine_status = MachineStatus::WaitingFulfill;
            }
        } else {
            LiveMachine::add_machine_id(&mut live_machines.online_machine, committee_upload_info.machine_id.clone());
            machine_info.machine_status = MachineStatus::Online;
            machine_info.reward_deadline = current_era + REWARD_DURATION;
        }

        MachinesInfo::<T>::insert(committee_upload_info.machine_id.clone(), machine_info.clone());
        LiveMachines::<T>::put(live_machines);

        // NOTE: Must be after MachinesInfo change, which depend on machine_info
        if let MachineStatus::Online = machine_info.machine_status {
            Self::change_server_room_gpu(&committee_upload_info.machine_id, true);
            Self::change_pos_gpu_by_online(&committee_upload_info.machine_id, true);
            Self::update_snap_by_online_status(committee_upload_info.machine_id, true);
        }

        return Ok(())
    }

    // 当委员会达成统一意见，拒绝机器时，机器状态改为委员会拒绝。并记录拒绝时间。
    fn lc_refuse_machine(machine_id: MachineId) -> Result<(), ()> {
        // 拒绝用户绑定，需要清除存储
        let mut machine_info = Self::machines_info(&machine_id);
        let now = <frame_system::Module<T>>::block_number();
        let mut sys_info = Self::sys_info();

        // 惩罚5%，并将机器ID移动到LiveMachine的补充质押中。
        let slash = Perbill::from_rational_approximation(5u64, 100u64) * machine_info.stake_amount;
        machine_info.stake_amount = machine_info.stake_amount - slash;

        sys_info.total_stake = sys_info.total_stake.checked_sub(&slash).unwrap();

        machine_info.machine_status = MachineStatus::CommitteeRefused(now);
        MachinesInfo::<T>::insert(&machine_id, machine_info);

        let mut live_machines = Self::live_machines();
        LiveMachine::rm_machine_id(&mut live_machines.booked_machine, &machine_id);
        LiveMachine::add_machine_id(&mut live_machines.refused_machine, machine_id);
        LiveMachines::<T>::put(live_machines);

        SysInfo::<T>::put(sys_info);

        Ok(())
    }
}

impl<T: Config> RTOps for Pallet<T> {
    type MachineId = MachineId;
    type MachineStatus = MachineStatus<T::BlockNumber>;
    type AccountId = T::AccountId;
    type BalanceOf = BalanceOf<T>;

    fn change_machine_status(
        machine_id: &MachineId,
        new_status: MachineStatus<T::BlockNumber>,
        renter: Option<Self::AccountId>,
        rent_duration: Option<u64>,
    ) {
        let mut machine_info = Self::machines_info(machine_id);
        let mut sys_info = Self::sys_info();
        let mut stash_machine = Self::stash_machines(&machine_info.machine_stash);

        machine_info.machine_status = new_status.clone();
        machine_info.last_machine_renter = renter;

        let machine_gpu_num = machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;

        match new_status {
            MachineStatus::Rented => {
                // 机器创建成功
                Self::update_snap_by_rent_status(machine_id.to_vec(), true);

                machine_info.total_rented_times += 1;

                if let Some(new_gpu_num) = sys_info.total_rented_gpu.checked_add(machine_gpu_num) {
                    sys_info.total_rented_gpu = new_gpu_num;
                } else {
                    debug::error!("Add sysInfo total rented gpu failed");
                }

                if let Some(new_gpu_num) = stash_machine.total_rented_gpu.checked_add(machine_gpu_num) {
                    stash_machine.total_rented_gpu = new_gpu_num;
                } else {
                    debug::error!("Add stashMachine total rented gpu failed");
                }

                Self::change_pos_gpu_by_rent(machine_id, true);
            },
            // 租用结束 或 租用失败(半小时无确认)
            MachineStatus::Online => {
                if rent_duration.is_some() {
                    // 租用结束
                    Self::update_snap_by_rent_status(machine_id.to_vec(), false);
                    machine_info.total_rented_duration += rent_duration.unwrap();

                    if let Some(new_gpu_num) = sys_info.total_rented_gpu.checked_sub(machine_gpu_num) {
                        sys_info.total_rented_gpu = new_gpu_num;
                    } else {
                        debug::error!("Sub sysInfo total rented gpu failed");
                    };
                    if let Some(new_gpu_num) = stash_machine.total_rented_gpu.checked_sub(machine_gpu_num) {
                        stash_machine.total_rented_gpu = new_gpu_num;
                    } else {
                        debug::error!("Sub stashMachine total rented gpu failed");
                    };

                    Self::change_pos_gpu_by_rent(machine_id, false);
                }
            },
            _ => {},
        }

        // 改变租用时长或者租用次数
        StashMachines::<T>::insert(&machine_info.machine_stash, stash_machine);
        SysInfo::<T>::put(sys_info);
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

    fn mt_machine_offline(machine_id: MachineId) {
        Self::machine_offline(machine_id);
    }
}

// RPC
impl<T: Config> Module<T> {
    pub fn get_total_staker_num() -> u64 {
        let all_stash = Self::get_all_stash();
        return all_stash.len() as u64
    }

    pub fn get_op_info() -> RpcSysInfo<BalanceOf<T>> {
        let sys_info = Self::sys_info();
        RpcSysInfo {
            total_gpu_num: sys_info.total_gpu_num,
            total_rented_gpu: sys_info.total_rented_gpu,
            total_staker: Self::get_total_staker_num(),
            total_calc_points: sys_info.total_calc_points,
            total_stake: sys_info.total_stake,
            total_rent_fee: sys_info.total_rent_fee,
            total_burn_fee: sys_info.total_burn_fee,
        }
    }

    pub fn get_staker_info(account: impl EncodeLike<T::AccountId>) -> RpcStakerInfo<BalanceOf<T>, T::BlockNumber> {
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
    pub fn get_machine_info(machine_id: MachineId) -> RPCMachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        let machine_info = Self::machines_info(&machine_id);
        RPCMachineInfo {
            machine_owner: machine_info.machine_stash,
            bonding_height: machine_info.bonding_height,
            stake_amount: machine_info.stake_amount,
            machine_status: machine_info.machine_status,
            total_rented_duration: machine_info.total_rented_duration,
            total_rented_times: machine_info.total_rented_times,
            total_rent_fee: machine_info.total_rent_fee,
            total_burn_fee: machine_info.total_burn_fee,
            machine_info_detail: machine_info.machine_info_detail,
            reward_committee: machine_info.reward_committee,
            reward_deadline: machine_info.reward_deadline,
        }
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
