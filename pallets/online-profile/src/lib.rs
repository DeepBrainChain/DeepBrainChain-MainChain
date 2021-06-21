// 机器单卡质押数量：
//   n = min(n1, n2)
//   n1 = 5w RMB 等值DBC
//   n2 = 100000 DBC (卡数 <= 10000)
//   n2 = 100000 * (10000/卡数) (卡数>10000)
// 在线奖励数量:
//   1th month:     10^8;                   Phase0  30 day
//   Next 2 year:   4 * 10^8;               Phase1  730 day
//   Next 9 month:  4 * 10^8 * (9 / 12);    Phase2  270 day
//   Next 5 year:   5 * 10^7;               Phase3  1825 day
//   Next 5 years:  2.5 * 10^7;             Phase4  1825 day
// 机器得分如何计算：
//   机器相对标准配置得到算力点数。机器实际得分 = 算力点数 + 算力点数 * 集群膨胀系数 + 算力点数 * 30%
//   因此，机器被租用时，机器实际得分 = 算力点数 * (1 + 集群膨胀系数 + 30%租用膨胀系数)

// TODO: era结束时重新计算得分, 如果有会影响得分的改变，放到列表中，等era结束进行计算

#![cfg_attr(not(feature = "std"), no_std)]

use codec::EncodeLike;
use frame_support::{
    dispatch::DispatchResultWithPostInfo,
    pallet_prelude::*,
    traits::{Currency, Get, LockIdentifier, LockableCurrency, OnUnbalanced, WithdrawReasons},
    weights::Weight,
    IterableStorageMap,
};
use frame_system::pallet_prelude::*;
use online_profile_machine::{LCOps, RTOps};
use pallet_identity::Data;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_core::crypto::Public;
use sp_runtime::{
    traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, Verify},
    Perbill, SaturatedConversion,
};
use sp_std::{
    collections::{btree_set::BTreeSet, vec_deque::VecDeque},
    convert::{TryFrom, TryInto},
    prelude::*,
    str,
};

pub mod grade_inflation;
pub mod op_types;
pub mod rpc_types;

pub use op_types::*;
pub use rpc_types::*;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub const PALLET_LOCK_ID: LockIdentifier = *b"oprofile";
pub const REPORTER_LOCK_ID: LockIdentifier = *b"reporter";
pub const MAX_UNLOCKING_CHUNKS: usize = 32;
pub const ONE_DBC: u64 = 1000_000_000_000_000;
// pub const BLOCK_PER_ERA: u64 = 2880;
pub const BLOCK_PER_ERA: u64 = 100; // TODO: 测试网一天设置为100个块

// 惩罚发生后，有48小时的时间提交议案取消惩罚
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachinesSlash<AccountId, BlockNumber, Balance> {
    pub reporter: AccountId,
    pub reporter_time: BlockNumber,
    pub slash_reason: SlashReason,
    pub slash_amount: Balance,
    pub unapplied_slash: u32, // 记录多少个阶段的奖励被扣除
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum SlashReason {
    MinuteOffline3, // 3min 掉线
    MinuteOffline7, // 7min 掉线
    DaysOffline2,   // 2days 掉线
    DaysOffline5,   // 5days 掉线
}

impl Default for SlashReason {
    fn default() -> Self {
        SlashReason::MinuteOffline3
    }
}

// 质押者总览自己当前状态状态
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct StakerMachine<Balance> {
    pub total_machine: Vec<MachineId>, // 用户绑定的所有机器，不与机器状态有关
    pub online_machine: Vec<MachineId>,
    pub total_calc_points: u64, // 用户的机器总得分，不给算集群膨胀系数与在线奖励
    pub total_gpu_num: u64,
    pub total_claimed_reward: Balance,
    pub can_claim_reward: Balance,      // 用户可以立即领取的奖励
    pub left_reward: VecDeque<Balance>, // 存储最多150个Era的奖励(150天将全部释放)，某个Era的数值等于 [0.99 * 0.75 * 当天该用户的所有奖励]
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineInfo<AccountId: Ord, BlockNumber, Balance> {
    pub controller: AccountId,             // 绑定机器的人
    pub machine_owner: AccountId, // 允许用户绑定跟自己机器ID不一样的，奖励发放给machine_owner
    pub machine_renter: Option<AccountId>, // 当前机器的租用者
    pub bonding_height: BlockNumber, // 记录机器第一次绑定的时间
    pub stake_amount: Balance,
    pub machine_status: MachineStatus<BlockNumber>,
    pub machine_info_detail: MachineInfoDetail, // 委员会提交的机器信息
    pub machine_price: u64, // 租用价格。设置3080的分数对应的价格为1000(可设置)元，其他机器的价格根据3080的价格，按照算力值进行计算的比例进行计算
    pub reward_committee: Vec<AccountId>, // 列表中的委员将分得用户奖励
    pub reward_deadline: BlockNumber, // 列表中委员分得奖励结束时间
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MachineStatus<BlockNumber> {
    MachineSelfConfirming,
    CommitteeVerifying,
    CommitteeRefused(BlockNumber),      // 委员会拒绝机器上线
    WaitingFulfill,                     // 补交质押
    Online,                             // 正在上线，且未被租用
    StakerReportOffline(BlockNumber),   // 机器管理者报告机器已下线
    ReporterReportOffline(BlockNumber), // 报告人报告机器下线
    Creating, // 机器被租用，虚拟机正在被创建，等待用户提交机器创建完成的信息
    Rented,   // 已经被租用
}

impl<BlockNumber> Default for MachineStatus<BlockNumber> {
    fn default() -> Self {
        MachineStatus::MachineSelfConfirming
    }
}

// 只保存正常声明周期的Machine,删除掉的/绑定失败的不保存在该变量中
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct LiveMachine {
    pub bonding_machine: Vec<MachineId>, // 用户质押DBC并绑定机器，机器ID添加到本字段
    pub machine_confirmed: Vec<MachineId>, // 机器钱包发送确认信息之后，添加到本字段。该状态可以由lc分配订单
    pub booked_machine: Vec<MachineId>, // 当机器已经全部分配了委员会，则变为该状态。若lc确认机器失败(认可=不认可)则返回上一状态，重新分派订单
    pub online_machine: Vec<MachineId>, // 被委员会确认之后之后，机器上线
    pub fulfilling_machine: Vec<MachineId>, // 拒绝接入后变为该状态
}

impl LiveMachine {
    // 检查machine_id是否存
    fn machine_id_exist(&self, machine_id: &MachineId) -> bool {
        if let Ok(_) = self.bonding_machine.binary_search(machine_id) {
            return true;
        }
        if let Ok(_) = self.machine_confirmed.binary_search(machine_id) {
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
        false
    }

    fn add_machine_id(a_field: &mut Vec<MachineId>, machine_id: MachineId) {
        if let Err(index) = a_field.binary_search(&machine_id) {
            a_field.insert(index, machine_id);
        }
    }

    fn rm_machine_id(a_field: &mut Vec<MachineId>, machine_id: &MachineId) {
        if let Ok(index) = a_field.binary_search(machine_id) {
            a_field.remove(index);
        }
    }
}

// 标准GPU租用价格
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct StandardGpuPointPrice {
    pub gpu_point: u64,
    pub gpu_price: u64,
}

pub type SlashId = u64;
type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> = <<T as Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::NegativeImbalance;

// 即将被执行的罚款
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct PendingSlashInfo<AccountId, BlockNumber, Balance> {
    pub slash_who: AccountId,
    pub slash_time: BlockNumber,      // 惩罚被创建的时间
    pub unlock_amount: Balance,       // 执行惩罚前解绑的金额
    pub slash_amount: Balance,        // 执行惩罚的金额
    pub slash_exec_time: BlockNumber, // 惩罚被执行的时间
    pub reward_to: Vec<AccountId>,    // 奖励发放对象。如果为空，则惩罚到国库
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + dbc_price_ocw::Config
        + generic_func::Config
        + pallet_identity::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type BondingDuration: Get<EraIndex>;
        type ProfitReleaseDuration: Get<u64>; // 剩余75%线性释放时间长度(25%立即释放)
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    // 存储用户机器在线收益
    #[pallet::type_value]
    pub fn HistoryDepthDefault<T: Config>() -> u32 {
        150
    }

    #[pallet::storage]
    #[pallet::getter(fn history_depth)]
    pub(super) type HistoryDepth<T: Config> =
        StorageValue<_, u32, ValueQuery, HistoryDepthDefault<T>>;

    // 存储机器的最小质押量，单位DBC, 默认为100000DBC
    #[pallet::storage]
    #[pallet::getter(fn stake_per_gpu)]
    pub(super) type StakePerGPU<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    // 特定标准的显卡算力点数和租用价格(USD)
    #[pallet::storage]
    #[pallet::getter(fn standard_gpu_point_price)]
    pub(super) type StandardGPUPointPrice<T: Config> = StorageValue<_, StandardGpuPointPrice>;

    // 存储每个机器质押的等值USD上限, 单位 1x10^6 USD
    #[pallet::storage]
    #[pallet::getter(fn stake_usd_limit)]
    pub(super) type StakeUSDLimit<T: Config> = StorageValue<_, u64, ValueQuery>;

    // 存储每个用户在该模块中的总质押量
    #[pallet::storage]
    #[pallet::getter(fn user_total_stake)]
    pub(super) type UserTotalStake<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    // 当前系统中工作的GPU数量
    #[pallet::storage]
    #[pallet::getter(fn total_gpu_num)]
    pub(super) type TotalGPUNum<T: Config> = StorageValue<_, u64, ValueQuery>;

    // 当前系统中矿工数
    #[pallet::storage]
    #[pallet::getter(fn total_staker)]
    pub(super) type TotalStaker<T: Config> = StorageValue<_, u64, ValueQuery>;

    // 当前系统中总算力点数
    #[pallet::storage]
    #[pallet::getter(fn total_calc_points)]
    pub(super) type TotalCalcPoints<T: Config> = StorageValue<_, u64, ValueQuery>;

    // 机器质押的DBC总数量
    #[pallet::storage]
    #[pallet::getter(fn total_stake)]
    pub(super) type TotalStake<T: Config> = StorageValue<_, BalanceOf<T>, ValueQuery>;

    // stash 对应的controller
    #[pallet::storage]
    #[pallet::getter(fn stash_controller)]
    pub(super) type StashController<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    // controller控制的stash
    #[pallet::storage]
    #[pallet::getter(fn controller_stash)]
    pub(super) type ControllerStash<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    #[pallet::storage]
    #[pallet::getter(fn next_slash_id)]
    pub(super) type NextSlashId<T: Config> = StorageValue<_, SlashId, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pending_slash)]
    pub(super) type PendingSlash<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        SlashId,
        PendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    // 机器的详细信息,只有当所有奖励领取完才能删除该变量?
    #[pallet::storage]
    #[pallet::getter(fn machines_info)]
    pub type MachinesInfo<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn user_machines)]
    pub(super) type UserMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, StakerMachine<BalanceOf<T>>, ValueQuery>;

    // 存储活跃的机器
    #[pallet::storage]
    #[pallet::getter(fn live_machines)]
    pub type LiveMachines<T: Config> = StorageValue<_, LiveMachine, ValueQuery>;

    // 每过2880个block，增加1
    #[pallet::storage]
    #[pallet::getter(fn current_era)]
    pub type CurrentEra<T: Config> = StorageValue<_, EraIndex, ValueQuery>;

    // 存储每个Era机器的得分
    #[pallet::storage]
    #[pallet::getter(fn eras_machine_points)]
    pub(super) type ErasMachinePoints<T: Config> =
        StorageMap<_, Blake2_128Concat, EraIndex, EraMachinePoints<T::AccountId>>;

    // 在线奖励开始时间
    #[pallet::storage]
    #[pallet::getter(fn reward_start_era)]
    pub(super) type RewardStartEra<T: Config> = StorageValue<_, EraIndex>;

    // 第一个月奖励
    #[pallet::storage]
    #[pallet::getter(fn phase_0_reward_per_era)]
    pub(super) type Phase0RewardPerEra<T: Config> = StorageValue<_, BalanceOf<T>>;

    // 随后每年总奖励
    #[pallet::storage]
    #[pallet::getter(fn phase_1_reward_per_era)]
    pub(super) type Phase1RewardPerEra<T: Config> = StorageValue<_, BalanceOf<T>>;

    #[pallet::storage]
    #[pallet::getter(fn phase_2_reward_per_era)]
    pub(super) type Phase2RewardPerEra<T: Config> = StorageValue<_, BalanceOf<T>>;

    #[pallet::storage]
    #[pallet::getter(fn phase_3_reward_per_era)]
    pub(super) type Phase3RewardPerEra<T: Config> = StorageValue<_, BalanceOf<T>>;

    #[pallet::storage]
    #[pallet::getter(fn phase_4_reward_per_era)]
    pub(super) type Phase4RewardPerEra<T: Config> = StorageValue<_, BalanceOf<T>>;

    // 奖励数量：第一个月为1亿，之后每个月为3300万
    // 2年10个月之后，奖励数量减半，之后再五年，奖励减半
    #[pallet::storage]
    #[pallet::getter(fn reward_per_year)]
    pub(super) type RewardPerYear<T> = StorageValue<_, BalanceOf<T>>;

    // 每个Era的总奖励
    #[pallet::storage]
    #[pallet::getter(fn eras_total_reward)]
    pub(super) type ErasTotalReward<T: Config> =
        StorageMap<_, Blake2_128Concat, EraIndex, Option<BalanceOf<T>>>;

    // 等于RewardPerYear * (era_duration / year_duration)
    #[pallet::storage]
    #[pallet::getter(fn eras_staker_reward)]
    pub(super) type ErasStakerReward<T> =
        StorageMap<_, Blake2_128Concat, EraIndex, Option<BalanceOf<T>>>;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(block_number: T::BlockNumber) -> Weight {
            // 每个Era开始的时候，生成当前Era的快照，和下一个Era的快照
            // 每天(2880个块)执行一次
            if block_number.saturated_into::<u64>() % BLOCK_PER_ERA == 1 {
                let current_era: u32 =
                    (block_number.saturated_into::<u64>() / BLOCK_PER_ERA) as u32;
                CurrentEra::<T>::put(current_era);

                if current_era == 0 {
                    ErasMachinePoints::<T>::insert(0, EraMachinePoints { ..Default::default() });
                    ErasMachinePoints::<T>::insert(1, EraMachinePoints { ..Default::default() });
                }

                // 用当前的Era快照初始化下一个Era的信息
                let current_era_clipp = Self::eras_machine_points(current_era).unwrap();
                ErasMachinePoints::<T>::insert(current_era + 1, current_era_clipp);
            }
            0
        }

        fn on_finalize(block_number: T::BlockNumber) {
            let current_height = block_number.saturated_into::<u64>();

            // 在每个Era结束时执行奖励，发放到用户的Machine
            // 计算奖励，直接根据当前得分即可
            if current_height > 0 && current_height % BLOCK_PER_ERA == 0 {
                if let Err(_) = Self::distribute_reward() {
                    debug::error!("##### Failed to distribute reward");
                }
            }

            Self::check_and_exec_slash();
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

        // 设置单卡质押数量
        #[pallet::weight(0)]
        pub fn set_gpu_stake(
            origin: OriginFor<T>,
            stake_per_gpu: BalanceOf<T>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StakePerGPU::<T>::put(stake_per_gpu);
            Ok(().into())
        }

        // 设置单GPU质押量换算成USD的上限
        #[pallet::weight(0)]
        pub fn set_stake_usd_limit(
            origin: OriginFor<T>,
            stake_usd_limit: u64,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StakeUSDLimit::<T>::put(stake_usd_limit);
            Ok(().into())
        }

        // 设置标准GPU租用标准算力与标准价格
        #[pallet::weight(0)]
        pub fn set_standard_gpu_point_price(
            origin: OriginFor<T>,
            point_price: StandardGpuPointPrice,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StandardGPUPointPrice::<T>::put(point_price);
            Ok(().into())
        }

        // 由stash账户发起请求设置一个控制账户
        #[pallet::weight(10000)]
        pub fn set_controller(
            origin: OriginFor<T>,
            controller: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let stash = ensure_signed(origin)?;
            StashController::<T>::insert(stash.clone(), controller.clone());
            ControllerStash::<T>::insert(controller, stash);
            Ok(().into())
        }

        // 将machine_id添加到绑定队列,之后ocw工作，验证机器ID与钱包地址是否一致
        // 绑定需要质押first_bond_stake数量的DBC
        #[pallet::weight(10000)]
        pub fn bond_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashBond)?;

            debug::error!("##### MachineId is: {:?}", machine_id.clone());

            // 用户第一次绑定机器需要质押的数量
            let first_bond_stake = Self::stake_per_gpu();

            // 扣除10个Dbc作为交易手续费
            <generic_func::Module<T>>::pay_fixed_tx_fee(controller.clone())
                .map_err(|_| Error::<T>::PayTxFeeFailed)?;

            // 资金检查, 确保机器还没有被绑定过
            ensure!(
                <T as Config>::Currency::free_balance(&stash) > first_bond_stake,
                Error::<T>::BalanceNotEnough
            );
            let mut live_machines = Self::live_machines();
            ensure!(!live_machines.machine_id_exist(&machine_id), Error::<T>::MachineIdExist);

            // 更新质押
            Self::add_user_total_stake(&stash, first_bond_stake)
                .map_err(|_| Error::<T>::BalanceOverflow)?;

            // 添加到用户的机器列表
            let mut user_machines = Self::user_machines(&controller);
            if let Err(index) = user_machines.total_machine.binary_search(&machine_id) {
                user_machines.total_machine.insert(index, machine_id.clone());
                UserMachines::<T>::insert(&controller, user_machines);
            } else {
                return Err(Error::<T>::MachineInUserBonded.into());
            }

            // 添加到LiveMachine的bonding_machine字段
            LiveMachine::add_machine_id(&mut live_machines.bonding_machine, machine_id.clone());
            LiveMachines::<T>::put(live_machines);

            // 初始化MachineInfo, 并添加到MachinesInfo
            let machine_info = MachineInfo {
                controller: controller.clone(),
                machine_owner: stash,
                bonding_height: <frame_system::Module<T>>::block_number(),
                stake_amount: first_bond_stake,
                ..Default::default()
            };
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::BondMachine(
                controller.clone(),
                machine_id.clone(),
                first_bond_stake,
            ));
            Ok(().into())
        }

        // 机器没有成功上线，则需要在10天内手动执行rebond
        #[pallet::weight(10000)]
        pub fn rebond_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let _now = <frame_system::Module<T>>::block_number();
            let machine_info = Self::machines_info(&machine_id);

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);
            // ensure!(machine_info.machine_status == MachineStatus::CommitteeRefused(_))
            if let MachineStatus::CommitteeRefused(_refuse_time) = machine_info.machine_status {
                // 超过10天
                // if refuse_time - now > 28800u64.saturated_into::<T::BlockNumber>() {
                // return Err();
                // }
            } else {
                // return Err(Error::<T>::Notsta);
            }

            Ok(().into())
        }

        // 控制账户可以随意修改镜像名称
        #[pallet::weight(10000)]
        pub fn staker_change_images_name(
            origin: OriginFor<T>,
            machine_id: MachineId,
            new_images: Vec<ImageName>,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            // 查询机器Id是否在该账户的控制下
            let user_machines = Self::user_machines(&controller);
            if let Err(_) = user_machines.total_machine.binary_search(&machine_id) {
                return Err(Error::<T>::MachineIdNotBonded.into());
            }

            let mut machine_info = Self::machines_info(&machine_id);
            machine_info.machine_info_detail.staker_customize_info.images = new_images;

            MachinesInfo::<T>::insert(machine_id, machine_info);
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn staker_change_machine_info(
            origin: OriginFor<T>,
            machine_id: MachineId,
            upload_net: u64,
            download_net: u64,
            longitude: u64,
            latitude: u64,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            // 查询机器Id是否在该账户的控制下
            let user_machines = Self::user_machines(&controller);
            user_machines
                .total_machine
                .binary_search(&machine_id)
                .map_err(|_| Error::<T>::MachineIdNotBonded)?;

            let mut machine_info = Self::machines_info(&machine_id);
            match machine_info.machine_status {
                // 判断机器状态，如果机器未上线，不改变机器状态
                MachineStatus::MachineSelfConfirming | MachineStatus::CommitteeVerifying => {}
                // 如果机器已上线，则减少可修改次数
                _ => {
                    let left_change_time =
                        machine_info.machine_info_detail.staker_customize_info.left_change_time;
                    if left_change_time == 0 {
                        return Err(Error::<T>::StakerMaxChangeReached.into());
                    }
                    machine_info.machine_info_detail.staker_customize_info = StakerCustomizeInfo {
                        left_change_time: left_change_time - 1,
                        upload_net,
                        download_net,
                        longitude,
                        latitude,
                        images: machine_info.machine_info_detail.staker_customize_info.images,
                    }
                }
            }

            Ok(().into())
        }

        // 超过一年的机器可以在不使用的时候退出
        #[pallet::weight(10000)]
        pub fn claim_exit(
            origin: OriginFor<T>,
            _controller: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let _controller = ensure_signed(origin)?;
            Ok(().into())
        }

        // FIXME: 当前问题：如果是Vec<u8>的参数，前端会把账户类型转为公钥传入，而如果是用别名，则传得字符串不会被改
        // 验证签名的第二个参数需要是公钥
        // 该方法由controller账户发出
        // machine 设置 stash账户
        // MachineId对应的私钥对字符串进行加密： "machineIdstash", 其中，machineId为machineId字符串，stash为Stash账户字符串
        #[pallet::weight(10000)]
        pub fn machine_set_stash(
            origin: OriginFor<T>,
            machine_id: MachineId,
            stash_account: MachineId,
            sig: MachineId,
        ) -> DispatchResultWithPostInfo {
            let sender = ensure_signed(origin)?;

            let mut msg = Vec::new();
            msg.extend(machine_id.clone());
            msg.extend(stash_account.clone());

            // TODO: 确保Machine_id中已经有该machine_id

            // 验证签名是否为MachineId发出
            // if Self::verify_sig(msg.clone(), sig.clone(), machine_id.clone()) {
            let mut machine_info = Self::machines_info(&machine_id);

            let stash_account = Self::get_account_from_str(&stash_account)
                .ok_or(Error::<T>::ConvertMachineIdToWalletFailed)?;
            machine_info.machine_owner = stash_account;

            let mut live_machines = Self::live_machines();
            LiveMachine::rm_machine_id(&mut live_machines.bonding_machine, &machine_id);
            LiveMachine::add_machine_id(&mut live_machines.machine_confirmed, machine_id.clone());

            MachinesInfo::<T>::insert(machine_id.clone(), machine_info);
            LiveMachines::<T>::put(live_machines);
            // } else {
            //     return Err(Error::<T>::BadSignature.into());
            // }

            Ok(().into())
        }

        // 矿工领取奖励
        #[pallet::weight(10000)]
        pub fn staker_claim_rewards(
            origin: OriginFor<T>,
            controller: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let staker = ensure_signed(origin)?;
            ensure!(UserMachines::<T>::contains_key(&controller), Error::<T>::NotMachineController);
            let mut user_machine = Self::user_machines(&staker);
            <T as pallet::Config>::Currency::deposit_into_existing(
                &staker,
                user_machine.can_claim_reward,
            )
            .map_err(|_| Error::<T>::ClaimRewardFailed)?;

            user_machine.total_claimed_reward += user_machine.can_claim_reward;
            user_machine.can_claim_reward = 0u64.saturated_into();
            UserMachines::<T>::insert(&staker, user_machine);

            Ok(().into())
        }

        // 机器管理者报告机器下线
        #[pallet::weight(10000)]
        pub fn staker_report_offline(
            origin: OriginFor<T>,
            _machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let _staker = ensure_signed(origin)?;

            Ok(().into())
        }

        // 机器管理者报告机器上线
        #[pallet::weight(10000)]
        pub fn staker_report_online(
            origin: OriginFor<T>,
            _machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let _staker = ensure_signed(origin)?;

            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn cancle_slash(
            origin: OriginFor<T>,
            _machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

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
        Slash(T::AccountId, BalanceOf<T>),
        MissedSlash(T::AccountId, BalanceOf<T>),
    }

    #[pallet::error]
    pub enum Error<T> {
        BadSignature,
        MachineIdExist,
        MachineIdNotBonded,
        MachineInBonded,
        MachineInBonding,
        MachineInBooking,
        MachineInBooked,
        MachineInUserBonded,
        MachineStatusNotNormal,
        TokenNotBonded,
        BondedNotEnough,
        HttpDecodeError,
        BalanceNotEnough,
        NotMachineOwner,
        LedgerNotFound,
        NoMoreChunks,
        AlreadyAddedMachine,
        InsufficientValue,
        IndexOutOfRange,
        InvalidEraToReward,
        AccountNotSame,
        NotInBookingList,
        StakeNotEnough,
        NotMachineController,
        DBCPriceUnavailable,
        StakerMaxChangeReached,
        BalanceOverflow,
        PayTxFeeFailed,
        RewardPhaseOutOfRange,
        ClaimRewardFailed,
        MachineWalletMachineIdNotMatch,
        ConvertMachineIdToWalletFailed,
        NoStashBond,
    }
}

impl<T: Config> Pallet<T> {
    fn verify_sig(msg: Vec<u8>, sig: Vec<u8>, account: Vec<u8>) -> bool {
        match sp_core::sr25519::Signature::try_from(&sig[..]) {
            Ok(signature) => {
                let public = sp_core::sr25519::Public::from_slice(account.as_ref());
                return signature.verify(&msg[..], &public);
            }
            _ => return false,
        };
    }

    // 参考：primitives/core/src/crypto.rs: impl Ss58Codec for AccountId32
    // from_ss58check_with_version
    fn get_account_from_str(addr: &Vec<u8>) -> Option<T::AccountId> {
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

        T::AccountId::decode(&mut &account_id32[..]).ok()
    }

    // 质押DBC机制：[0, 10000] GPU: 100000 DBC per GPU
    // (10000, +) -> min( 100000 * 10000 / (10000 + n), 5w RMB DBC )
    pub fn calc_stake_amount(gpu_num: u32) -> Option<BalanceOf<T>> {
        let base_stake = Self::stake_per_gpu(); // 10_0000 DBC
        let one_dbc: BalanceOf<T> = 1000_000_000_000_000u64.saturated_into();

        // 计算5w RMB 等值DBC数量
        // dbc_amount = dbc_stake_usd_limit * 10^15 / dbc_price
        let dbc_price: BalanceOf<T> = <dbc_price_ocw::Module<T>>::avg_price()?.saturated_into();
        let stake_usd_limit: BalanceOf<T> = Self::stake_usd_limit().saturated_into();
        let dbc_amount = one_dbc.checked_mul(&stake_usd_limit)?;
        let dbc_amount = dbc_amount.checked_div(&dbc_price)?;

        // 当前成功加入系统的GPU数量
        let total_gpu_num = Self::total_gpu_num();

        if total_gpu_num <= 10_000 {
            // GPU数量小于10_000时，直接返回base_saturated_into() satura
            return Some(base_stake.min(dbc_amount));
        }

        // 当GPU数量大于10_000时
        // 100_000 * 10000 / gpu_num
        let dbc_amount2 =
            Perbill::from_rational_approximation(10_000u64, total_gpu_num) * base_stake;

        let stake_per_gpu = dbc_amount2.min(dbc_amount);
        stake_per_gpu.checked_mul(&gpu_num.saturated_into::<BalanceOf<T>>())
    }

    // 根据GPU数量和该机器算力点数，算出该机器价格
    pub fn calc_machine_price(machine_point: u64, gpu_num: u32) -> Option<u64> {
        let standard_gpu_point_price = Self::standard_gpu_point_price()?;
        // let standard_gpu_point = Self::standard_gpu_point()?;
        standard_gpu_point_price
            .gpu_price
            .checked_mul(10_000)?
            .checked_mul(machine_point)?
            .checked_mul(gpu_num as u64)?
            .checked_div(standard_gpu_point_price.gpu_point)?
            .checked_div(10_000u64)
    }

    // 获取机器最近n天的奖励
    pub fn remaining_n_eras_reward(_machine_id: MachineId, _recent_eras: u32) -> BalanceOf<T> {
        return 0u32.into();
    }

    // 在线奖励数量: TODO: 正式网将修改奖励时间
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

    fn add_user_total_stake(controller: &T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        let current_stake = Self::user_total_stake(controller);
        let next_stake = current_stake.checked_add(&amount).ok_or(())?;
        <T as pallet::Config>::Currency::set_lock(
            PALLET_LOCK_ID,
            controller,
            next_stake,
            WithdrawReasons::all(),
        );

        UserTotalStake::<T>::insert(controller, next_stake);
        // 改变总质押
        let total_stake = Self::total_stake().checked_add(&amount).ok_or(())?;
        TotalStake::<T>::put(total_stake);
        Ok(())
    }

    fn reduce_user_total_stake(controller: &T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        let current_stake = Self::user_total_stake(controller);
        let next_stake = current_stake.checked_sub(&amount).ok_or(())?;
        <T as pallet::Config>::Currency::set_lock(
            PALLET_LOCK_ID,
            controller,
            next_stake,
            WithdrawReasons::all(),
        );

        UserTotalStake::<T>::insert(controller, next_stake);
        // 改变总质押
        let total_stake = Self::total_stake().checked_sub(&amount).ok_or(())?;
        TotalStake::<T>::put(total_stake);
        Ok(())
    }

    // 因事件发生，更新某个机器得分。
    // 由于机器被添加到在线或者不在线，更新矿工机器得分与总得分
    // 因增加机器，在线获得的分数奖励，则修改下一天的快照
    // 如果机器掉线，则修改当天的快照，影响当天奖励
    fn update_staker_grades_by_online_machine(
        staker: T::AccountId,
        machine_id: MachineId,
        is_online: bool,
    ) {
        let current_era = Self::current_era();
        let machine_info = Self::machines_info(&machine_id);
        // let machine_base_calc_point = machine_info.machine_info_detail.committee_upload_info.calc_point;
        let machine_base_info = machine_info.machine_info_detail.committee_upload_info;

        let era_index = if is_online { current_era + 1 } else { current_era };

        let mut era_machine_point = Self::eras_machine_points(era_index).unwrap();
        let mut staker_machine = Self::user_machines(&staker);

        let mut staker_statistic = era_machine_point
            .staker_statistic
            .entry(staker.clone())
            .or_insert(StakerStatistics { ..Default::default() });

        // 用户之前的总得分
        let old_grade = staker_statistic.machine_total_calc_point
            + staker_statistic.inflation * staker_statistic.machine_total_calc_point
            + staker_statistic.rent_extra_grade;

        // 重新计算膨胀得分
        if is_online {
            staker_statistic.online_num += 1;
        } else {
            staker_statistic.online_num -= 1;
        }
        let bond_num = staker_statistic.online_num as u32;

        staker_statistic.inflation = if bond_num <= 1000 {
            Perbill::from_rational_approximation(bond_num, 10_000) // 线性增加, 最大10%
        } else {
            Perbill::from_rational_approximation(1000u64, 10_000) // max: 10%
        };

        // 新的机器算里得分之和
        if is_online {
            staker_statistic.machine_total_calc_point += machine_base_info.calc_point;

            if let Err(index) = staker_machine.online_machine.binary_search(&machine_id) {
                staker_machine.online_machine.insert(index, machine_id.clone());
            }
            staker_machine.total_calc_points += machine_base_info.calc_point;
            staker_machine.total_gpu_num += machine_base_info.gpu_num as u64;
            // TODO: 更新系统总统计
        } else {
            staker_statistic.machine_total_calc_point -= machine_base_info.calc_point;

            if let Ok(index) = staker_machine.online_machine.binary_search(&machine_id) {
                staker_machine.online_machine.remove(index);
            }
            staker_machine.total_calc_points -= machine_base_info.calc_point;
            staker_machine.total_gpu_num -= machine_base_info.gpu_num as u64;
        }

        let new_grade = staker_statistic.machine_total_calc_point
            + staker_statistic.inflation * staker_statistic.machine_total_calc_point
            + staker_statistic.rent_extra_grade;

        // 更新系统总得分
        let staker_statistic = (*staker_statistic).clone();
        era_machine_point.staker_statistic.insert(staker.clone(), staker_statistic);
        era_machine_point.total -= old_grade;
        era_machine_point.total += new_grade;

        UserMachines::<T>::insert(&staker, staker_machine);
        ErasMachinePoints::<T>::insert(&era_index, era_machine_point);
    }

    // 由于机器被租用，而更新得分
    // 机器被租用和退租都修改下一天得分
    fn update_staker_grades_by_rented_change(
        // staker: T::AccountId,
        machine_id: MachineId,
        is_rented: bool,
    ) {
        let era_index = Self::current_era() + 1;
        let machine_info = Self::machines_info(&machine_id);
        let machine_base_calc_point =
            machine_info.machine_info_detail.committee_upload_info.calc_point;

        let mut era_machine_point = Self::eras_machine_points(era_index).unwrap();
        let mut staker_statistic = era_machine_point
            .staker_statistic
            .entry(machine_info.controller.clone())
            .or_insert(StakerStatistics { ..Default::default() });

        // 某台机器被租用，则该机器得分多30%
        // 某台机器被退租，则该机器得分少30%
        let grade_change =
            Perbill::from_rational_approximation(30u64, 100u64) * machine_base_calc_point;
        if is_rented {
            staker_statistic.rent_extra_grade += grade_change;
            era_machine_point.total += grade_change;
        } else {
            staker_statistic.rent_extra_grade -= grade_change;
            era_machine_point.total -= grade_change;
        }

        let staker_statistic = (*staker_statistic).clone();
        era_machine_point
            .staker_statistic
            .insert(machine_info.controller.clone(), staker_statistic);
        ErasMachinePoints::<T>::insert(&era_index, era_machine_point);
    }

    // end_era分发奖励
    fn distribute_reward() -> Result<(), ()> {
        let current_era = Self::current_era();
        let current_rewward_per_era = Self::current_era_reward().ok_or(())?;

        let era_machine_point = Self::eras_machine_points(current_era).unwrap();
        // let user_machines = Self::user_machines()
        // 遍历列表，获得奖励
        let all_staker = Self::get_all_staker();
        for a_staker in all_staker {
            match era_machine_point.staker_statistic.get(&a_staker) {
                None => {
                    // 即使当天没有发放奖励，仍然有可能还存在未释放的奖励

                    // 如果没有任何该释放的奖励，则是否考虑清除该用户的变量
                }
                Some(staker_statistic) => {
                    // 记录用户获得的奖励和还未释放的奖励
                    let mut staker_machine = Self::user_machines(&a_staker);

                    let staker_actual_grade = staker_statistic.rent_extra_grade
                        + staker_statistic.machine_total_calc_point
                        + staker_statistic.inflation * staker_statistic.machine_total_calc_point;

                    let should_reward = Perbill::from_rational_approximation(
                        staker_actual_grade,
                        era_machine_point.total,
                    ) * current_rewward_per_era;

                    // 应该获得的奖励，应该按照25立即发放，75线性发放
                    // 25% 发放到用户的user_machine中，75也是。
                    // 1%的奖励发放给委员会的帐号里
                    let reward_to_committee =
                        Perbill::from_rational_approximation(1u64, 100u64) * should_reward;
                    // TODO: 应该按照机器当前得分占该用户的总得分的比例，来分奖励

                    let left_reward = should_reward - reward_to_committee;

                    let staker_get_now =
                        Perbill::from_rational_approximation(25u64, 100u64) * left_reward; // 用户立刻获得0.99 * 0.25的奖励
                    let staker_left_reward = left_reward - staker_get_now; // 剩余0.99 * 75%的奖励留作线性释放

                    // 矿工实际获得的奖励，还应该加上前面150天没有释放的奖励之和的1/150
                    let mut left_reward_sum = 0u64.saturated_into();
                    let left_reward = staker_machine.left_reward.clone(); // 剩余的75%的奖励
                    for a_left_reward in left_reward {
                        left_reward_sum += a_left_reward;
                    }
                    let linear_released: BalanceOf<T> =
                        Perbill::from_rational_approximation(1u64, 150u64) * left_reward_sum; // 应该线性释放的奖励

                    // 计算当前Era应该释放的奖励
                    staker_machine.can_claim_reward =
                        staker_machine.can_claim_reward + linear_released + staker_get_now;

                    staker_machine.left_reward.push_back(staker_left_reward);
                    // 如果长多超过了150，则第一天的已经发放完了
                    if staker_machine.left_reward.len() > 150 {
                        staker_machine.left_reward.pop_front();
                    }

                    UserMachines::<T>::insert(&a_staker, staker_machine);
                }
            }
        }

        return Ok(());
    }

    fn get_all_staker() -> Vec<T::AccountId> {
        <UserMachines<T> as IterableStorageMap<T::AccountId, _>>::iter()
            .map(|(staker, _)| staker)
            .collect::<Vec<_>>()
    }

    fn get_new_slash_id() -> SlashId {
        let slash_id = Self::next_slash_id();
        NextSlashId::<T>::put(slash_id + 1);
        return slash_id;
    }

    fn add_slash(who: T::AccountId, amount: BalanceOf<T>, reward_to: Vec<T::AccountId>) {
        let slash_id = Self::get_new_slash_id();
        let now = <frame_system::Module<T>>::block_number();
        PendingSlash::<T>::insert(
            slash_id,
            PendingSlashInfo {
                slash_who: who,
                slash_time: now,
                unlock_amount: amount,
                slash_amount: amount,
                slash_exec_time: now + 5760u32.saturated_into::<T::BlockNumber>(),
                reward_to,
            },
        );
    }

    // 获得所有被惩罚的订单列表
    fn get_slash_id() -> BTreeSet<SlashId> {
        <PendingSlash<T> as IterableStorageMap<SlashId, _>>::iter()
            .map(|(slash_id, _)| slash_id)
            .collect::<BTreeSet<_>>()
    }

    // 检查fulfilling list，如果超过10天，则清除记录，退还质押
    fn _check_and_clean_refused_machine() {
        let now = <frame_system::Module<T>>::block_number();

        let live_machines = Self::live_machines();
        for a_machine in live_machines.fulfilling_machine {
            let machine_info = Self::machines_info(&a_machine);

            if let MachineStatus::CommitteeRefused(refuse_time) = machine_info.machine_status {
                if refuse_time - now >= 28800u64.saturated_into::<T::BlockNumber>() {}
            }
        }
    }

    // 检查并执行slash
    fn check_and_exec_slash() {
        let now = <frame_system::Module<T>>::block_number();

        let pending_slash_id = Self::get_slash_id();
        for a_slash_id in pending_slash_id {
            let a_slash_info = Self::pending_slash(&a_slash_id);
            if now >= a_slash_info.slash_exec_time {
                let _ = Self::reduce_user_total_stake(
                    &a_slash_info.slash_who,
                    a_slash_info.unlock_amount,
                );

                // 如果reward_to为0，则将币转到国库
                if a_slash_info.reward_to.len() == 0 {
                    if <T as pallet::Config>::Currency::can_slash(
                        &a_slash_info.slash_who,
                        a_slash_info.slash_amount,
                    ) {
                        let (imbalance, missing) = <T as pallet::Config>::Currency::slash(
                            &a_slash_info.slash_who,
                            a_slash_info.slash_amount,
                        );
                        Self::deposit_event(Event::Slash(
                            a_slash_info.slash_who.clone(),
                            a_slash_info.slash_amount,
                        ));
                        Self::deposit_event(Event::MissedSlash(
                            a_slash_info.slash_who,
                            missing.clone(),
                        ));
                        T::Slash::on_unbalanced(imbalance);
                    }
                } else {
                    // TODO: reward_to将获得slash的奖励
                }
            }
        }
    }
}

// 审查委员会可以执行的操作
impl<T: Config> LCOps for Pallet<T> {
    type MachineId = MachineId;
    type AccountId = T::AccountId;
    type CommitteeUploadInfo = CommitteeUploadInfo;

    // 委员会订阅了一个机器ID
    // 将机器状态从ocw_confirmed_machine改为booked_machine，同时将机器状态改为booked
    fn lc_booked_machine(id: MachineId) {
        let mut live_machines = Self::live_machines();

        LiveMachine::rm_machine_id(&mut live_machines.machine_confirmed, &id);
        LiveMachine::add_machine_id(&mut live_machines.booked_machine, id.clone());
        LiveMachines::<T>::put(live_machines);

        let mut machine_info = Self::machines_info(&id);
        machine_info.machine_status = MachineStatus::CommitteeVerifying;
        MachinesInfo::<T>::insert(&id, machine_info);
    }

    // 由于委员会没有达成一致，需要重新返回到bonding_machine
    fn lc_revert_booked_machine(id: MachineId) {
        let mut live_machines = Self::live_machines();

        LiveMachine::rm_machine_id(&mut live_machines.booked_machine, &id);
        LiveMachine::add_machine_id(&mut live_machines.machine_confirmed, id.clone());

        let mut machine_info = Self::machines_info(&id);
        machine_info.machine_status = MachineStatus::MachineSelfConfirming;
        MachinesInfo::<T>::insert(&id, machine_info);
    }

    // 当多个委员会都对机器进行了确认之后，添加机器信息，并更新机器得分
    // 机器被成功添加, 则添加上可以获取收益的委员会
    fn lc_confirm_machine(
        reported_committee: Vec<T::AccountId>,
        committee_upload_info: CommitteeUploadInfo,
    ) -> Result<(), ()> {
        debug::warn!("##### CommitteeUploadInfo is: {:?}", committee_upload_info);

        let mut machine_info = Self::machines_info(&committee_upload_info.machine_id);
        machine_info.machine_info_detail.committee_upload_info = committee_upload_info.clone();
        machine_info.reward_committee = reported_committee;

        let stake_need = Self::calc_stake_amount(committee_upload_info.gpu_num).ok_or(())?;
        // 改变用户的绑定数量。如果用户余额足够，则直接质押。否则将机器状态改为补充质押
        let stake_need = stake_need.checked_sub(&machine_info.stake_amount);
        if let None = stake_need {
            // 表示不用补交质押
            machine_info.machine_status = MachineStatus::Online;
        } else {
            let stake_need = stake_need.unwrap();
            if <T as Config>::Currency::free_balance(&machine_info.machine_owner) > stake_need {
                Self::add_user_total_stake(&machine_info.machine_owner, stake_need)?;
                machine_info.machine_status = MachineStatus::Online;
            } else {
                machine_info.machine_status = MachineStatus::WaitingFulfill;
            }
        }

        // 添加机器价格
        let machine_price = Self::calc_machine_price(
            committee_upload_info.calc_point,
            committee_upload_info.gpu_num,
        )
        .ok_or(())?;
        machine_info.machine_price = machine_price;

        MachinesInfo::<T>::insert(committee_upload_info.machine_id.clone(), machine_info.clone());

        let mut user_machines = Self::user_machines(&machine_info.machine_owner);
        user_machines.total_calc_points += committee_upload_info.calc_point;
        user_machines.total_gpu_num += committee_upload_info.gpu_num as u64;
        UserMachines::<T>::insert(&machine_info.machine_owner, user_machines);

        let mut live_machines = Self::live_machines();
        LiveMachine::rm_machine_id(
            &mut live_machines.booked_machine,
            &committee_upload_info.machine_id,
        );
        LiveMachine::add_machine_id(
            &mut live_machines.online_machine,
            committee_upload_info.machine_id.clone(),
        );
        LiveMachines::<T>::put(live_machines);

        let total_gpu_num = Self::total_gpu_num();
        TotalGPUNum::<T>::put(total_gpu_num + committee_upload_info.gpu_num as u64);

        let total_calc_points = Self::total_calc_points();
        TotalCalcPoints::<T>::put(total_calc_points + committee_upload_info.calc_point);

        Self::update_staker_grades_by_online_machine(
            machine_info.machine_owner,
            committee_upload_info.machine_id,
            true,
        );
        return Ok(());
    }

    // TODO: 当委员会达成统一意见，拒绝机器时，机器状态改为补充质押。并记录拒绝时间。
    fn lc_refuse_machine(machine_id: MachineId) -> Result<(), ()> {
        // 拒绝用户绑定，需要清除存储
        let mut machine_info = Self::machines_info(&machine_id);
        let now = <frame_system::Module<T>>::block_number();

        // 惩罚5%，并将机器ID移动到LiveMachine的补充质押中。
        let slash = Perbill::from_rational_approximation(5u64, 100u64) * machine_info.stake_amount;
        machine_info.stake_amount = machine_info.stake_amount - slash;

        Self::add_slash(machine_info.controller.clone(), slash, Vec::new());

        machine_info.machine_status = MachineStatus::CommitteeRefused(now);
        MachinesInfo::<T>::insert(&machine_id, machine_info);

        let mut live_machines = Self::live_machines();
        LiveMachine::rm_machine_id(&mut live_machines.booked_machine, &machine_id);
        LiveMachine::add_machine_id(&mut live_machines.fulfilling_machine, machine_id);
        LiveMachines::<T>::put(live_machines);

        Ok(())
    }
}

impl<T: Config> RTOps for Pallet<T> {
    type MachineId = MachineId;
    type MachineStatus = MachineStatus<T::BlockNumber>;
    type AccountId = T::AccountId;

    fn change_machine_status(
        machine_id: &MachineId,
        new_status: MachineStatus<T::BlockNumber>,
        renter: Self::AccountId,
        is_rent: bool,
    ) {
        let mut machine_info = Self::machines_info(machine_id);
        if machine_info.machine_status == new_status {
            return;
        }
        machine_info.machine_status = new_status;
        machine_info.machine_renter = Some(renter.clone());
        MachinesInfo::<T>::insert(machine_id, machine_info);
        Self::update_staker_grades_by_rented_change(machine_id.to_vec(), is_rent)
    }
}

// RPC
impl<T: Config> Module<T> {
    pub fn get_total_staker_num() -> u64 {
        let all_staker = Self::get_all_staker();
        return all_staker.len() as u64;
    }

    pub fn get_op_info() -> SysInfo<BalanceOf<T>> {
        SysInfo {
            total_gpu_num: Self::total_gpu_num(),
            total_staker: Self::total_staker(),
            total_calc_points: Self::total_calc_points(),
            total_stake: Self::total_stake(),
        }
    }

    pub fn get_staker_info(account: impl EncodeLike<T::AccountId>) -> StakerInfo<BalanceOf<T>> {
        let staker_info = Self::user_machines(account);

        StakerInfo {
            calc_points: staker_info.total_calc_points,
            gpu_num: staker_info.total_gpu_num,
            total_reward: staker_info.total_claimed_reward + staker_info.can_claim_reward,
        }
    }

    // TODO:
    pub fn get_staker_list(_start: u64, _end: u64) -> Vec<T::AccountId> {
        Self::get_all_staker()
    }

    pub fn get_staker_identity(account: impl EncodeLike<T::AccountId>) -> Vec<u8> {
        let account_info = <pallet_identity::Module<T>>::identity(account);
        if let None = account_info {
            return Vec::new();
        }
        let account_info = account_info.unwrap();

        match account_info.info.display {
            Data::Raw(out) => return out,
            _ => return Vec::new(),
        }
    }

    // 返回total_page
    pub fn get_staker_list_info(
        cur_page: u64,
        per_page: u64,
    ) -> Vec<StakerListInfo<BalanceOf<T>, T::AccountId>> {
        let temp_account = Self::get_all_staker();
        let mut out = Vec::new();

        if temp_account.len() == 0 {
            return out;
        }

        let cur_page = cur_page as usize;
        let per_page = per_page as usize;
        let page_start = cur_page * per_page;
        let mut page_end = page_start + per_page;

        if page_start >= temp_account.len() {
            return out;
        }

        if page_end >= temp_account.len() {
            page_end = temp_account.len() - 1;
        }

        for a_account in temp_account[page_start..page_end].into_iter() {
            let staker_info = Self::user_machines(a_account.clone());
            let identity = Self::get_staker_identity(a_account.clone());

            out.push(StakerListInfo {
                staker_name: identity,
                staker_account: a_account.clone(),
                calc_points: staker_info.total_calc_points,
                gpu_num: staker_info.total_gpu_num,
                gpu_rent_rate: 0u64,
                total_reward: staker_info.total_claimed_reward + staker_info.can_claim_reward,
            })
        }

        return out;
    }

    // 获取机器列表
    pub fn get_machine_list() -> LiveMachine {
        Self::live_machines()
    }

    // 获取机器详情
    pub fn get_machine_info(
        machine_id: MachineId,
    ) -> RPCMachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>> {
        let machine_info = Self::machines_info(&machine_id);
        RPCMachineInfo {
            machine_owner: machine_info.machine_owner,
            bonding_height: machine_info.bonding_height,
            stake_amount: machine_info.stake_amount,
            // machine_status: machine_info.machine_status,
            machine_info_detail: machine_info.machine_info_detail,
            machine_price: machine_info.machine_price,
            // reward_committee: machine_info.reward_committee,
            reward_deadline: machine_info.reward_deadline,
        }
    }
}
