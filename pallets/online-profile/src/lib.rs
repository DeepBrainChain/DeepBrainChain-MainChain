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
    traits::{Currency, Get, LockableCurrency},
    weights::Weight,
    IterableStorageDoubleMap, IterableStorageMap,
};
use frame_system::pallet_prelude::*;
use online_profile_machine::{DbcPrice, LCOps, ManageCommittee, OPRPCQuery, RTOps};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::crypto::Public;
use sp_runtime::{
    traits::{CheckedAdd, CheckedMul, CheckedSub, Verify},
    Perbill, SaturatedConversion,
};
use sp_std::{
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
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

/// stash账户总览自己当前状态
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct StashMachine<Balance> {
    /// stash账户绑定的所有机器，不与机器状态有关
    pub total_machine: Vec<MachineId>,
    /// stash账户绑定的处于在线状态的机器
    pub online_machine: Vec<MachineId>,
    /// 在线机器总得分，不给算集群膨胀系数与在线奖励
    pub total_calc_points: u64,
    /// 在线机器的总GPU个数
    pub total_gpu_num: u64,
    /// 被租用的GPU个数
    pub total_rented_gpu: u64,
    /// 总计领取奖励数量
    pub total_claimed_reward: Balance,
    /// 目前能够领取奖励的数量
    pub can_claim_reward: Balance,
    /// 每个Era剩余的99% * 75%奖励。存储最多150个Era的奖励(150天将全部释放)
    pub linear_release_reward: VecDeque<Balance>,
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
    /// 当前机器的租用者
    pub machine_renter: Option<AccountId>,
    /// 记录机器第一次绑定上线的时间
    pub bonding_height: BlockNumber,
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
    pub reward_deadline: BlockNumber,
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
    ReporterReportOffline(BlockNumber),
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
    pub online_machine: Vec<MachineId>, // TODO: 是不是加一个offline的
    /// 委员会同意上线，但是由于stash账户质押不够，需要补充质押
    pub fulfilling_machine: Vec<MachineId>,
    /// 被委员会拒绝的机器（10天内还能重新申请上线）
    pub refused_machine: Vec<MachineId>,
}

impl LiveMachine {
    /// 检查machine_id是否存
    fn machine_id_exist(&self, machine_id: &MachineId) -> bool {
        if let Ok(_) = self.bonding_machine.binary_search(machine_id) {
            return true;
        }
        if let Ok(_) = self.confirmed_machine.binary_search(machine_id) {
            return true;
        }
        if let Ok(_) = self.booked_machine.binary_search(machine_id) {
            return true;
        }
        if let Ok(_) = self.online_machine.binary_search(machine_id) {
            return true;
        }
        if let Ok(_) = self.fulfilling_machine.binary_search(machine_id) {
            return true;
        }
        if let Ok(_) = self.refused_machine.binary_search(machine_id) {
            return true;
        }
        false
    }

    /// 向LiveMachine某个字段添加machine_id
    fn add_machine_id(a_field: &mut Vec<MachineId>, machine_id: MachineId) {
        if let Err(index) = a_field.binary_search(&machine_id) {
            a_field.insert(index, machine_id);
        }
    }

    /// 从LiveMachine某个字段删除machine_id
    fn rm_machine_id(a_field: &mut Vec<MachineId>, machine_id: &MachineId) {
        if let Ok(index) = a_field.binary_search(machine_id) {
            a_field.remove(index);
        }
    }

    /// 获取所有MachineId
    fn all_machine_id(self) -> Vec<MachineId> {
        let mut out = Vec::new();
        out.extend(self.bonding_machine);
        out.extend(self.confirmed_machine);
        out.extend(self.booked_machine);
        out.extend(self.online_machine);
        out.extend(self.fulfilling_machine);
        out.extend(self.refused_machine);
        out
    }
}

/// 标准GPU租用价格
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct StandardGpuPointPrice {
    /// 标准GPU算力点数
    pub gpu_point: u64,
    /// 标准GPu价格
    pub gpu_price: u64,
}

type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

/// 在线奖励系统信息统计
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct SysInfoDetail<Balance> {
    /// 在线机器的GPU的总数
    pub total_gpu_num: u64,
    /// 被租用机器的GPU的总数
    pub total_rented_gpu: u64,
    /// 系统中总stash账户数量(有机器成功上线)
    pub total_staker: u64,
    /// 系统中上线的总算力点数
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
    /// 在线机器算力点数
    pub online_gpu_calc_points: u64,
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
        type ManageCommittee: ManageCommittee<
            AccountId = Self::AccountId,
            BalanceOf = BalanceOf<Self>,
        >;
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

    /// 用户在本模块中的总质押量
    #[pallet::storage]
    #[pallet::getter(fn user_total_stake)]
    pub(super) type UserTotalStake<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

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
        StorageDoubleMap<_, Blake2_128Concat, i64, Blake2_128Concat, i64, PosInfo, ValueQuery>;

    /// stash 对应的 controller
    #[pallet::storage]
    #[pallet::getter(fn stash_controller)]
    pub(super) type StashController<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    /// controller 控制的 stash
    #[pallet::storage]
    #[pallet::getter(fn controller_stash)]
    pub(super) type ControllerStash<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    /// 机器的详细信息
    #[pallet::storage]
    #[pallet::getter(fn machines_info)]
    pub type MachinesInfo<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    /// stash账户下所有机器统计
    #[pallet::storage]
    #[pallet::getter(fn stash_machines)]
    pub(super) type StashMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, StashMachine<BalanceOf<T>>, ValueQuery>;

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
    pub(super) type ErasMachinePoints<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        EraIndex,
        BTreeMap<MachineId, MachineGradeStatus<T::AccountId>>,
    >;

    /// 在线奖励开始时间
    #[pallet::storage]
    #[pallet::getter(fn reward_start_era)]
    pub(super) type RewardStartEra<T: Config> = StorageValue<_, EraIndex>;

    /// 第一阶段每Era奖励DBC数量
    #[pallet::storage]
    #[pallet::getter(fn phase_0_reward_per_era)]
    pub(super) type Phase0RewardPerEra<T: Config> = StorageValue<_, BalanceOf<T>>;

    /// 第二阶段每Era奖励DBC数量
    #[pallet::storage]
    #[pallet::getter(fn phase_1_reward_per_era)]
    pub(super) type Phase1RewardPerEra<T: Config> = StorageValue<_, BalanceOf<T>>;

    /// 第三阶段每Era奖励DBC数量
    #[pallet::storage]
    #[pallet::getter(fn phase_2_reward_per_era)]
    pub(super) type Phase2RewardPerEra<T: Config> = StorageValue<_, BalanceOf<T>>;

    /// 第四阶段每Era奖励DBC数量
    #[pallet::storage]
    #[pallet::getter(fn phase_3_reward_per_era)]
    pub(super) type Phase3RewardPerEra<T: Config> = StorageValue<_, BalanceOf<T>>;

    /// 第五阶段每Era奖励DBC数量
    #[pallet::storage]
    #[pallet::getter(fn phase_4_reward_per_era)]
    pub(super) type Phase4RewardPerEra<T: Config> = StorageValue<_, BalanceOf<T>>;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(block_number: T::BlockNumber) -> Weight {
            // Era开始时，生成当前Era和下一个Era的快照
            // 每个Era(2880个块)执行一次
            if block_number.saturated_into::<u64>() % BLOCK_PER_ERA == 1 {
                let current_era: u32 =
                    (block_number.saturated_into::<u64>() / BLOCK_PER_ERA) as u32;
                CurrentEra::<T>::put(current_era);

                if current_era == 0 {
                    ErasStashPoints::<T>::insert(0, EraStashPoints { ..Default::default() });
                    ErasStashPoints::<T>::insert(1, EraStashPoints { ..Default::default() });
                    let init_value: BTreeMap<MachineId, MachineGradeStatus<T::AccountId>> =
                        BTreeMap::new();
                    ErasMachinePoints::<T>::insert(0, init_value.clone());
                    ErasMachinePoints::<T>::insert(1, init_value);
                } else {
                    // 用当前的Era快照初始化下一个Era的信息
                    let current_era_stash_snapshot = Self::eras_stash_points(current_era).unwrap();
                    ErasStashPoints::<T>::insert(current_era + 1, current_era_stash_snapshot);
                    let current_era_machine_snapshot =
                        Self::eras_machine_points(current_era).unwrap();
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
                if let Err(_) = Self::distribute_reward() {
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
        pub fn set_reward_start_era(
            origin: OriginFor<T>,
            reward_start_era: EraIndex,
        ) -> DispatchResultWithPostInfo {
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
                0 => Phase0RewardPerEra::<T>::put(reward_per_era),
                1 => Phase1RewardPerEra::<T>::put(reward_per_era),
                2 => Phase2RewardPerEra::<T>::put(reward_per_era),
                3 => Phase3RewardPerEra::<T>::put(reward_per_era),
                4 => Phase4RewardPerEra::<T>::put(reward_per_era),
                _ => return Err(Error::<T>::RewardPhaseOutOfRange.into()),
            }
            Ok(().into())
        }

        /// 设置单卡质押DBC数量
        #[pallet::weight(0)]
        pub fn set_gpu_stake(
            origin: OriginFor<T>,
            stake_per_gpu: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StakePerGPU::<T>::put(stake_per_gpu);
            Ok(().into())
        }

        /// 单GPU质押量等价USD的上限
        #[pallet::weight(0)]
        pub fn set_stake_usd_limit(
            origin: OriginFor<T>,
            stake_usd_limit: u64,
        ) -> DispatchResultWithPostInfo {
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

        /// stash账户设置一个控制账户
        #[pallet::weight(10000)]
        pub fn set_controller(
            origin: OriginFor<T>,
            controller: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let stash = ensure_signed(origin)?;
            // 不允许多个stash指定同一个controller
            ensure!(
                !<ControllerStash<T>>::contains_key(&controller),
                Error::<T>::AlreadyController
            );

            StashController::<T>::insert(stash.clone(), controller.clone());
            ControllerStash::<T>::insert(controller, stash);
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
            let sig_stash_account = Self::get_account_from_str(&sig_stash_account)
                .ok_or(Error::<T>::ConvertMachineIdToWalletFailed)?;
            ensure!(sig_stash_account == stash, Error::<T>::MachineStashNotEqualControllerStash);

            // 验证签名是否为MachineId发出
            if Self::verify_sig(msg.clone(), sig.clone(), machine_id.clone()).is_none() {
                return Err(Error::<T>::BadSignature.into());
            }

            // 用户绑定机器需要质押一张显卡的DBC
            let stake_amount =
                Self::calc_stake_amount(1).ok_or(Error::<T>::CalcStakeAmountFailed)?;

            // 扣除10个Dbc作为交易手续费
            <generic_func::Module<T>>::pay_fixed_tx_fee(controller.clone())
                .map_err(|_| Error::<T>::PayTxFeeFailed)?;

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

            Self::add_user_total_stake(&stash, stake_amount)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;

            ControllerMachines::<T>::insert(&controller, controller_machines);
            StashMachines::<T>::insert(&stash, stash_machines);
            LiveMachines::<T>::put(live_machines);
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::BondMachine(
                controller.clone(),
                machine_id.clone(),
                stake_amount,
            ));
            Ok(().into())
        }

        /// 控制账户添加机器信息: 经纬度*10^4取整
        /// 符号：东经+,西经-；北纬+,南纬-,
        #[pallet::weight(10000)]
        pub fn add_machine_info(
            origin: OriginFor<T>,
            machine_id: MachineId,
            customize_machine_info: StakerCustomizeInfo,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            if customize_machine_info.telecom_operators.len() == 0
                || customize_machine_info.images.len() == 0
            {
                return Err(Error::<T>::TelecomAndImageIsNull.into());
            }

            // 查询机器Id是否在该账户的控制下
            let mut machine_info = Self::machines_info(&machine_id);
            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);

            match machine_info.machine_status {
                MachineStatus::AddingCustomizeInfo
                | MachineStatus::CommitteeVerifying
                | MachineStatus::CommitteeRefused(_)
                | MachineStatus::WaitingFulfill
                | MachineStatus::StakerReportOffline(_, _) => {
                    machine_info.machine_info_detail.staker_customize_info = customize_machine_info;
                }
                _ => {
                    return Err(Error::<T>::NotAllowedChangeMachineInfo.into());
                }
            }

            let mut live_machines = Self::live_machines();
            if let Ok(index) = live_machines.bonding_machine.binary_search(&machine_id) {
                live_machines.bonding_machine.remove(index);
                if let Err(index) = live_machines.confirmed_machine.binary_search(&machine_id) {
                    live_machines.confirmed_machine.insert(index, machine_id.clone());
                }
                LiveMachines::<T>::put(live_machines);
            }

            machine_info.machine_status = MachineStatus::DistributingOrder;
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Ok(().into())
        }

        /// 控制账户可以修改机器镜像信息，修改不限次数
        #[pallet::weight(10000)]
        pub fn change_images_name(
            origin: OriginFor<T>,
            machine_id: MachineId,
            new_images: Vec<ImageName>,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            let mut machine_info = Self::machines_info(&machine_id);
            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);

            machine_info.machine_info_detail.staker_customize_info.images = new_images;
            MachinesInfo::<T>::insert(machine_id, machine_info);
            Ok(().into())
        }

        /// 机器处于补交质押状态时，需要补交质押才能上线
        #[pallet::weight(10000)]
        pub fn fulfill_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            let mut machine_info = Self::machines_info(&machine_id);
            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);

            let stake_need = Self::calc_stake_amount(
                machine_info.machine_info_detail.committee_upload_info.gpu_num,
            )
            .ok_or(Error::<T>::CalcStakeAmountFailed)?;

            if machine_info.stake_amount < stake_need {
                let extra_stake = stake_need - machine_info.stake_amount;

                Self::add_user_total_stake(&machine_info.machine_stash, extra_stake)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;

                machine_info.stake_amount = stake_need;
            }
            machine_info.machine_status = MachineStatus::Online;

            MachinesInfo::<T>::insert(&machine_id, machine_info);
            Ok(().into())
        }

        /// 如果绑定失败，会扣除5%的DBC（5000），要在10天内手动执行rebond并补充质押
        #[pallet::weight(10000)]
        pub fn rebond_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            // let now = <frame_system::Module<T>>::block_number();
            let mut machine_info = Self::machines_info(&machine_id);

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);
            match machine_info.machine_status {
                MachineStatus::CommitteeRefused(_) => {}
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
            let stash_account =
                Self::controller_stash(&controller).ok_or(Error::<T>::NoStashAccount)?;
            ensure!(
                StashMachines::<T>::contains_key(&stash_account),
                Error::<T>::NotMachineController
            );
            let mut stash_machine = Self::stash_machines(&stash_account);

            <T as pallet::Config>::Currency::deposit_into_existing(
                &stash_account,
                stash_machine.can_claim_reward,
            )
            .map_err(|_| Error::<T>::ClaimRewardFailed)?;

            stash_machine.total_claimed_reward += stash_machine.can_claim_reward;
            stash_machine.can_claim_reward = 0u64.saturated_into();
            StashMachines::<T>::insert(&stash_account, stash_machine);

            Ok(().into())
        }

        /// 控制账户报告机器下线
        #[pallet::weight(10000)]
        pub fn controller_report_offline(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut machine_info = Self::machines_info(&machine_id);
            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);

            // TODO: 检查机器状态，在online之后，还应该是这种状态
            machine_info.machine_status =
                MachineStatus::StakerReportOffline(now, Box::new(machine_info.machine_status));

            // TODO: 应该影响机器打分
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::change_pos_gpu_by_online(&machine_id, false);

            Ok(().into())
        }

        /// 控制账户报告机器上线
        #[pallet::weight(10000)]
        pub fn controller_report_online(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            let mut machine_info = Self::machines_info(&machine_id);
            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);

            machine_info.machine_status = MachineStatus::Online;

            // TODO: 根据机器掉线时间进行惩罚

            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::change_pos_gpu_by_online(&machine_id, true);
            Ok(().into())
        }

        /// 超过365天的机器可以在距离上次租用10天，且没被租用时退出
        #[pallet::weight(10000)]
        pub fn claim_exit(
            origin: OriginFor<T>,
            _controller: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let _controller = ensure_signed(origin)?;
            Ok(().into())
        }

        /// 满足365天可以申请重新质押，退回质押币
        ///
        /// 在系统中上线满365天之后，可以按当时机器需要的质押数量，重新入网。多余的币解绑
        /// 在重新上线之后，下次再执行本操作，需要等待365天
        #[pallet::weight(10000)]
        pub fn rebond_online_machine(
            origin: OriginFor<T>,
            _machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let _controller = ensure_signed(origin)?;
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
        TelecomAndImageIsNull,
    }
}

impl<T: Config> Pallet<T> {
    /// 特定位置GPU上线/下线
    fn change_pos_gpu_by_online(machine_id: &MachineId, is_online: bool) {
        let machine_info = Self::machines_info(machine_id);

        let longitude = machine_info.machine_info_detail.staker_customize_info.longitude;
        let latitude = machine_info.machine_info_detail.staker_customize_info.latitude;
        let gpu_num = machine_info.machine_info_detail.committee_upload_info.gpu_num;
        let calc_point = machine_info.machine_info_detail.committee_upload_info.calc_point;

        let mut pos_gpu_info = Self::pos_gpu_info(longitude, latitude);

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

        let longitude = machine_info.machine_info_detail.staker_customize_info.longitude;
        let latitude = machine_info.machine_info_detail.staker_customize_info.latitude;
        let gpu_num = machine_info.machine_info_detail.committee_upload_info.gpu_num;

        let mut pos_gpu_info = Self::pos_gpu_info(longitude, latitude);
        if is_rented {
            pos_gpu_info.rented_gpu += gpu_num as u64;
        } else {
            pos_gpu_info.rented_gpu -= gpu_num as u64;
        }

        PosGPUInfo::<T>::insert(longitude, latitude, pos_gpu_info);
    }

    // 检查fulfilling list，如果超过10天，则清除记录，退还质押
    fn clean_refused_machine() {
        let mut live_machines = Self::live_machines();
        if live_machines.refused_machine.len() == 0 {
            return;
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

                        if let Err(_) = T::ManageCommittee::change_stake(
                            &machine_info.machine_stash,
                            machine_info.stake_amount,
                            false,
                        ) {
                            debug::error!("Reduce user stake failed");
                            continue;
                        }
                        if let Some(value) =
                            sys_info.total_stake.checked_sub(&machine_info.stake_amount)
                        {
                            sys_info.total_stake = value;
                        } else {
                            debug::error!("Reduce total stake failed");
                            continue;
                        }

                        let mut controller_machines =
                            Self::controller_machines(&machine_info.controller);
                        if let Ok(index) = controller_machines.binary_search(&a_machine) {
                            controller_machines.remove(index);
                        }

                        let mut stash_machines = Self::stash_machines(&machine_info.machine_stash);
                        if let Ok(index) = stash_machines.total_machine.binary_search(&a_machine) {
                            stash_machines.total_machine.remove(index);
                        }

                        ControllerMachines::<T>::insert(
                            &machine_info.controller,
                            controller_machines,
                        );
                        StashMachines::<T>::insert(&machine_info.machine_stash, stash_machines);
                        MachinesInfo::<T>::remove(a_machine);
                    }
                }
                _ => {}
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
        let pubkey_hex: Result<Vec<u8>, _> = (0..pubkey_str.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&pubkey_str[i..i + 2], 16))
            .collect();
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
            return None;
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
            base_stake =
                Perbill::from_rational_approximation(10_000u64, sys_info.total_gpu_num) * base_stake
        }

        let stake_usd_limit = Self::stake_usd_limit();
        let stake_limit = T::DbcPrice::get_dbc_amount_by_value(stake_usd_limit)?;

        return base_stake.min(stake_limit).checked_mul(&gpu_num.saturated_into::<BalanceOf<T>>());
    }

    /// 根据GPU数量和该机器算力点数，计算该机器相比标准配置的价格
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
            return None;
        }

        let era_duration = current_era - reward_start_era;

        let reward_per_era = if era_duration < 30 {
            Self::phase_0_reward_per_era()
        } else if era_duration < 30 + 730 {
            Self::phase_1_reward_per_era()
        } else if era_duration < 30 + 730 + 270 {
            Self::phase_2_reward_per_era()
        } else if era_duration < 30 + 730 + 270 + 1825 {
            Self::phase_3_reward_per_era()
        } else {
            Self::phase_4_reward_per_era()
        };

        return reward_per_era;
    }

    // 扣除n天剩余奖励
    fn _slash_nday_reward(
        _controller: T::AccountId,
        _machine_id: MachineId,
        _amount: BalanceOf<T>,
    ) {
    }

    fn _validator_slash() {}

    fn add_user_total_stake(who: &T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        T::ManageCommittee::change_stake(&who, amount, true)?;

        // 改变总质押
        let mut sys_info = Self::sys_info();
        sys_info.total_stake = sys_info.total_stake.checked_add(&amount).ok_or(())?;
        SysInfo::<T>::put(sys_info);
        Ok(())
    }

    fn _reduce_user_total_stake(who: &T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        T::ManageCommittee::change_stake(&who, amount, false)?;

        // 改变总质押
        let mut sys_info = Self::sys_info();
        sys_info.total_stake = sys_info.total_stake.checked_sub(&amount).ok_or(())?;
        SysInfo::<T>::put(sys_info);

        Ok(())
    }

    // TODO: 完善unwarp
    /// 由于机器在线或者下线，更新矿工机器得分与总得分:
    ///     机器上线，则修改下一天的得分快照
    ///     机器掉线，则修改当天与下一天的快照，影响今后奖励
    fn update_staker_grades_by_online_machine(
        stash_account: T::AccountId,
        machine_id: MachineId,
        is_online: bool,
    ) {
        let machine_info = Self::machines_info(&machine_id);
        let machine_base_info = machine_info.machine_info_detail.committee_upload_info;

        let mut stash_machine = Self::stash_machines(&stash_account);
        let mut sys_info = Self::sys_info();

        Self::update_snap_by_online_status(machine_id.clone(), is_online);

        if is_online {
            if let Err(index) = stash_machine.online_machine.binary_search(&machine_id) {
                stash_machine.online_machine.insert(index, machine_id.clone());
            }
            stash_machine.total_calc_points += machine_base_info.calc_point;
            stash_machine.total_gpu_num += machine_base_info.gpu_num as u64;

            sys_info.total_calc_points += machine_base_info.calc_point;
            sys_info.total_gpu_num += machine_base_info.gpu_num as u64;
        } else {
            if let Ok(index) = stash_machine.online_machine.binary_search(&machine_id) {
                stash_machine.online_machine.remove(index);
            }
            stash_machine.total_calc_points -= machine_base_info.calc_point;
            stash_machine.total_gpu_num -= machine_base_info.gpu_num as u64;

            sys_info.total_calc_points -= machine_base_info.calc_point;
            sys_info.total_gpu_num -= machine_base_info.gpu_num as u64;
        }

        // NOTE: 5000张卡开启银河竞赛
        if !Self::galaxy_is_on() && sys_info.total_gpu_num > 5000 {
            GalaxyIsOn::<T>::put(true);
        }

        SysInfo::<T>::put(sys_info);
        StashMachines::<T>::insert(&stash_account, stash_machine);
    }

    fn update_snap_by_online_status(machine_id: MachineId, is_online: bool) {
        let machine_info = Self::machines_info(&machine_id);
        let current_era = Self::current_era();
        let mut current_era_stash_snap = Self::eras_stash_points(current_era).unwrap();
        let mut next_era_stash_snap = Self::eras_stash_points(current_era + 1).unwrap();

        let mut current_era_machine_snap = Self::eras_machine_points(current_era).unwrap(); // FIXME
        let mut next_era_machine_snap = Self::eras_machine_points(current_era + 1).unwrap();

        next_era_stash_snap.change_machine_online_status(
            machine_info.machine_stash.clone(),
            machine_info.machine_info_detail.committee_upload_info.gpu_num as u64,
            machine_info.machine_info_detail.committee_upload_info.calc_point,
            is_online,
        );

        if is_online {
            next_era_machine_snap.insert(
                machine_id.clone(),
                MachineGradeStatus {
                    basic_grade: machine_info.machine_info_detail.committee_upload_info.calc_point,
                    is_rented: false,
                    reward_account: machine_info.reward_committee,
                },
            );
        } else {
            current_era_stash_snap.change_machine_online_status(
                machine_info.machine_stash.clone(),
                machine_info.machine_info_detail.committee_upload_info.gpu_num as u64,
                machine_info.machine_info_detail.committee_upload_info.calc_point,
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
    }

    fn update_snap_by_rent_status(machine_id: MachineId, is_rented: bool) {
        let machine_info = Self::machines_info(&machine_id);
        let current_era = Self::current_era();
        let mut current_era_stash_snap = Self::eras_stash_points(current_era).unwrap();
        let mut next_era_stash_snap = Self::eras_stash_points(current_era + 1).unwrap();

        let mut current_era_machine_snap = Self::eras_machine_points(current_era).unwrap();
        let mut next_era_machine_snap = Self::eras_machine_points(current_era + 1).unwrap();

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
    }

    // 根据机器得分快照，和委员会膨胀分数，计算应该奖励
    // end_era分发奖励
    fn distribute_reward() -> Result<(), ()> {
        let current_era = Self::current_era();
        let start_era = if current_era > 150 { current_era - 150 } else { 0u32 };
        let live_machines = Self::live_machines();
        let all_machine_id = live_machines.all_machine_id();

        // 释放75%的奖励
        for era_index in start_era..=current_era {
            let era_machine_snap = Self::eras_machine_points(era_index).unwrap();
            let era_stash_snap = Self::eras_stash_points(era_index).unwrap();
            let era_reward = Self::current_era_reward();
            if era_reward.is_none() {
                continue;
            }
            let era_reward = era_reward.unwrap();

            for machine_id in &all_machine_id {
                let machine_info = Self::machines_info(&machine_id);
                let mut stash_machine = Self::stash_machines(&machine_info.machine_stash);

                let machine_snap = era_machine_snap.get(machine_id);
                if machine_snap.is_none() {
                    continue;
                }
                let machine_snap = machine_snap.unwrap();

                let stash_snap = era_stash_snap.staker_statistic.get(&machine_info.machine_stash);
                if stash_snap.is_none() {
                    continue;
                }
                let stash_snap = stash_snap.unwrap();

                let machine_actual_grade = machine_snap.machine_actual_grade(stash_snap.inflation);

                // 该Era机器获得的总奖励
                let machine_total_reward = Perbill::from_rational_approximation(
                    machine_actual_grade as u64,
                    era_stash_snap.total as u64,
                ) * era_reward;

                let linear_reward_part =
                    Perbill::from_rational_approximation(75u64, 100u64) * machine_total_reward;

                let release_now = if era_index == current_era {
                    // 记录剩余的75%奖励
                    if stash_machine.linear_release_reward.len() == 150 {
                        stash_machine.linear_release_reward.pop_front();
                    }
                    stash_machine.linear_release_reward.push_back(linear_reward_part);

                    machine_total_reward - linear_reward_part
                } else {
                    // 剩余75%的1/150
                    Perbill::from_rational_approximation(1u32, 150u32) * linear_reward_part
                    // linear_reward_part / 150u32.into()
                };

                if machine_snap.reward_account.len() == 0 {
                    stash_machine.can_claim_reward += release_now;
                    // 没有委员会来分，则全部奖励给stash账户
                } else {
                    // 99% 分给stash账户
                    let release_to_stash =
                        Perbill::from_rational_approximation(99u64, 100u64) * release_now;
                    stash_machine.can_claim_reward += release_to_stash;

                    // 剩下分给committee
                    let release_to_committee = release_now - release_to_stash;
                    let committee_each_get = Perbill::from_rational_approximation(
                        1u64,
                        machine_snap.reward_account.len() as u64,
                    ) * release_to_committee;

                    for a_committee in machine_snap.reward_account.clone() {
                        T::ManageCommittee::add_reward(a_committee, committee_each_get);
                    }
                }

                StashMachines::<T>::insert(&machine_info.machine_stash, stash_machine);
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
        debug::warn!("CommitteeUploadInfo is: {:?}", committee_upload_info);

        let mut machine_info = Self::machines_info(&committee_upload_info.machine_id);
        let mut live_machines = Self::live_machines();
        let mut sys_info = Self::sys_info();

        LiveMachine::rm_machine_id(
            &mut live_machines.booked_machine,
            &committee_upload_info.machine_id,
        );

        machine_info.machine_info_detail.committee_upload_info = committee_upload_info.clone();
        machine_info.reward_committee = reported_committee;

        // 改变用户的绑定数量。如果用户余额足够，则直接质押。否则将机器状态改为补充质押
        let stake_need = Self::calc_stake_amount(committee_upload_info.gpu_num).ok_or(())?;
        // if let Some(stake_need) = stake_need.checked_sub(&machine_info.stake_amount) {
        if let Ok(_) = T::ManageCommittee::change_stake(
            &machine_info.machine_stash,
            stake_need - machine_info.stake_amount,
            true,
        ) {
            LiveMachine::add_machine_id(
                &mut live_machines.online_machine,
                committee_upload_info.machine_id.clone(),
            );
            machine_info.machine_status = MachineStatus::Online;
            sys_info.total_stake += stake_need - machine_info.stake_amount;
        } else {
            LiveMachine::add_machine_id(
                &mut live_machines.fulfilling_machine,
                committee_upload_info.machine_id.clone(),
            );
            machine_info.machine_status = MachineStatus::WaitingFulfill;
        }

        MachinesInfo::<T>::insert(committee_upload_info.machine_id.clone(), machine_info.clone());
        LiveMachines::<T>::put(live_machines);
        SysInfo::<T>::put(sys_info);

        Self::change_pos_gpu_by_online(&committee_upload_info.machine_id, true);

        // // TODO: 增加机器数量
        // let pos_gpu_info = Self::pos_gpu_info(machine_info.);

        Self::update_staker_grades_by_online_machine(
            machine_info.machine_stash,
            committee_upload_info.machine_id,
            true,
        );
        return Ok(());
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

        machine_info.machine_status = new_status.clone();
        machine_info.machine_renter = renter;

        match new_status {
            MachineStatus::Rented => {
                // 机器创建成功
                Self::update_snap_by_rent_status(machine_id.to_vec(), true);

                machine_info.total_rented_times += 1;

                sys_info.total_rented_gpu +=
                    machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;

                Self::change_pos_gpu_by_rent(machine_id, true);
            }
            // 租用结束 或 租用失败(半小时无确认)
            MachineStatus::Online => {
                if rent_duration.is_some() {
                    // 租用结束
                    Self::update_snap_by_rent_status(machine_id.to_vec(), false);
                    machine_info.total_rented_duration += rent_duration.unwrap();

                    sys_info.total_rented_gpu -=
                        machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;

                    Self::change_pos_gpu_by_rent(machine_id, false);
                }
            }
            _ => {}
        }

        // 改变租用时长或者租用次数
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

// RPC
impl<T: Config> Module<T> {
    pub fn get_total_staker_num() -> u64 {
        let all_stash = Self::get_all_stash();
        return all_stash.len() as u64;
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

    pub fn get_staker_info(
        account: impl EncodeLike<T::AccountId>,
    ) -> RpcStakerInfo<BalanceOf<T>, T::BlockNumber> {
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
    pub fn get_machine_info(
        machine_id: MachineId,
    ) -> RPCMachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>> {
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
    pub fn get_pos_gpu_info() -> Vec<(i64, i64, PosInfo)> {
        <PosGPUInfo<T> as IterableStorageDoubleMap<i64, i64, PosInfo>>::iter()
            .map(|(k1, k2, v)| (k1, k2, v))
            .collect()
    }
}
