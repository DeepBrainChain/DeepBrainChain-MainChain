#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use codec::{alloc::string::ToString, Decode, Encode};
use generic_func::{ItemList, MachineId};
use sp_core::H256;
use sp_io::hashing::blake2_128;
use sp_runtime::{traits::Saturating, Perbill, RuntimeDebug};
use sp_std::{
    collections::{btree_map::BTreeMap, vec_deque::VecDeque},
    ops::{Add, Sub},
    prelude::Box,
    vec,
    vec::Vec,
};

/// 2880 blocks per era
pub const BLOCK_PER_ERA: u64 = 2880;
/// Reward duration for committee (Era)
pub const REWARD_DURATION: u32 = 365 * 2;
/// Rebond frequency, 1 year
pub const REBOND_FREQUENCY: u32 = 365 * 2880;
/// Max Slash Threshold: 120h, 5 era
pub const MAX_SLASH_THRESHOLD: u32 = 2880 * 5;
/// PendingSlash will be exec in two days
pub const TWO_DAY: u32 = 5760;

pub type EraIndex = u32;
pub type TelecomName = Vec<u8>;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineRecentRewardInfo<AccountId, Balance> {
    // machine total reward(committee reward included)
    pub machine_stash: AccountId,
    pub recent_machine_reward: VecDeque<Balance>,
    pub recent_reward_sum: Balance,

    pub reward_committee_deadline: EraIndex,
    pub reward_committee: Vec<AccountId>,
}

// NOTE: Call order of add_new_reward and get_..released is very important
// Add new reward first, then calc committee/stash released reward
impl<AccountId, Balance> MachineRecentRewardInfo<AccountId, Balance>
where
    Balance: Default + Clone + Add<Output = Balance> + Sub<Output = Balance> + Copy,
{
    pub fn add_new_reward(&mut self, reward_amount: Balance) {
        let mut reduce = Balance::default();

        if self.recent_machine_reward.len() == 150 {
            reduce = self.recent_machine_reward.pop_front().unwrap();
            self.recent_machine_reward.push_back(reward_amount);
        } else {
            self.recent_machine_reward.push_back(reward_amount);
        }

        self.recent_reward_sum = self.recent_reward_sum + reward_amount - reduce;
    }
}

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

impl<B: Saturating + Copy> StashMachine<B> {
    // 新加入的机器，放到total_machine中
    pub fn new_bonding(&mut self, machine_id: MachineId) {
        ItemList::add_item(&mut self.total_machine, machine_id);
    }

    pub fn change_rent_fee(&mut self, amount: B, is_burn: bool) {
        if is_burn {
            self.total_burn_fee = self.total_burn_fee.saturating_add(amount);
        } else {
            self.total_rent_fee = self.total_rent_fee.saturating_add(amount);
        }
    }
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

impl<A: Ord + Default, B: Default, C: Copy + Default + Saturating> MachineInfo<A, B, C> {
    pub fn new_bonding(controller: A, stash: A, now: B, init_stake_per_gpu: C) -> Self {
        Self {
            controller,
            machine_stash: stash,
            bonding_height: now,
            init_stake_per_gpu,
            stake_amount: init_stake_per_gpu,
            machine_status: MachineStatus::AddingCustomizeInfo,
            ..Default::default()
        }
    }

    pub fn can_add_customize_info(&self) -> bool {
        matches!(
            self.machine_status,
            MachineStatus::AddingCustomizeInfo
                | MachineStatus::CommitteeVerifying
                | MachineStatus::CommitteeRefused(_)
                | MachineStatus::WaitingFulfill
                | MachineStatus::StakerReportOffline(_, _)
        )
    }

    pub fn change_rent_fee(&mut self, amount: C, is_burn: bool) {
        if is_burn {
            self.total_burn_fee = self.total_burn_fee.saturating_add(amount);
        } else {
            self.total_rent_fee = self.total_rent_fee.saturating_add(amount);
        }
    }
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
    pub fn is_bonding(&self, machine_id: &MachineId) -> bool {
        self.bonding_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_confirmed(&self, machine_id: &MachineId) -> bool {
        self.confirmed_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_booked(&self, machine_id: &MachineId) -> bool {
        self.booked_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_online(&self, machine_id: &MachineId) -> bool {
        self.online_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_fulfilling(&self, machine_id: &MachineId) -> bool {
        self.fulfilling_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_refused(&self, machine_id: &MachineId) -> bool {
        self.refused_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_rented(&self, machine_id: &MachineId) -> bool {
        self.rented_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_offline(&self, machine_id: &MachineId) -> bool {
        self.offline_machine.binary_search(machine_id).is_ok()
    }
    pub fn is_refused_mut_hardware(&self, machine_id: &MachineId) -> bool {
        self.refused_machine.binary_search(machine_id).is_ok()
    }

    /// Check if machine_id exist
    pub fn machine_id_exist(&self, machine_id: &MachineId) -> bool {
        self.is_bonding(machine_id)
            || self.is_confirmed(machine_id)
            || self.is_booked(machine_id)
            || self.is_online(machine_id)
            || self.is_fulfilling(machine_id)
            || self.is_refused(machine_id)
            || self.is_rented(machine_id)
            || self.is_offline(machine_id)
            || self.is_refused_mut_hardware(machine_id)
    }

    // 添加到LiveMachine的bonding_machine字段
    pub fn new_bonding(&mut self, machine_id: MachineId) {
        ItemList::add_item(&mut self.bonding_machine, machine_id);
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
    /// How much should stake when apply_slash_review
    pub slash_review_stake: Balance,
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
pub struct UserMutHardwareStakeInfo<Balance, BlockNumber> {
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

impl<Balance: Saturating + Copy> SysInfoDetail<Balance> {
    pub fn change_stake(&mut self, amount: Balance, is_add: bool) {
        if is_add {
            self.total_stake = self.total_stake.saturating_add(amount);
        } else {
            self.total_stake = self.total_stake.saturating_sub(amount);
        }
    }

    pub fn change_rent_fee(&mut self, amount: Balance, is_burn: bool) {
        if is_burn {
            self.total_burn_fee = self.total_burn_fee.saturating_add(amount);
        } else {
            self.total_rent_fee = self.total_rent_fee.saturating_add(amount);
        }
    }
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

impl PosInfo {
    pub fn is_rented(&mut self, is_rented: bool, gpu_num: u32) {
        if is_rented {
            self.rented_gpu = self.rented_gpu.saturating_add(gpu_num as u64);
        } else {
            self.rented_gpu = self.rented_gpu.saturating_sub(gpu_num as u64);
        }
    }

    pub fn is_online(&mut self, is_online: bool, gpu_num: u32, calc_point: u64) {
        let gpu_num = gpu_num as u64;
        if is_online {
            self.online_gpu = self.online_gpu.saturating_add(gpu_num);
            self.online_gpu_calc_points = self.online_gpu_calc_points.saturating_add(calc_point);
        } else {
            self.online_gpu = self.online_gpu.saturating_sub(gpu_num);
            self.online_gpu_calc_points = self.online_gpu_calc_points.saturating_sub(calc_point);

            self.offline_gpu = self.offline_gpu.saturating_add(gpu_num);
        }
    }
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

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OPPendingSlashReviewInfo<AccountId, Balance, BlockNumber> {
    pub applicant: AccountId,
    pub staked_amount: Balance,
    pub apply_time: BlockNumber,
    pub expire_time: BlockNumber,
    pub reason: Vec<u8>,
}

// For RPC
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RpcStakerInfo<Balance, BlockNumber, AccountId> {
    pub stash_statistic: StashMachine<Balance>,
    pub bonded_machines: Vec<MachineBriefInfo<BlockNumber, AccountId>>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct MachineBriefInfo<BlockNumber, AccountId> {
    pub machine_id: MachineId,
    pub gpu_num: u32,
    pub calc_point: u64,
    pub machine_status: MachineStatus<BlockNumber, AccountId>,
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct MachineInfoDetail {
    pub committee_upload_info: CommitteeUploadInfo,
    pub staker_customize_info: StakerCustomizeInfo,
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CommitteeUploadInfo {
    pub machine_id: MachineId,
    pub gpu_type: Vec<u8>, // GPU型号
    pub gpu_num: u32,      // GPU数量
    pub cuda_core: u32,    // CUDA core数量
    pub gpu_mem: u64,      // GPU显存
    pub calc_point: u64,   // 算力值
    pub sys_disk: u64,     // 系统盘大小
    pub data_disk: u64,    // 数据盘大小
    pub cpu_type: Vec<u8>, // CPU型号
    pub cpu_core_num: u32, // CPU内核数
    pub cpu_rate: u64,     // CPU频率
    pub mem_num: u64,      // 内存数

    pub rand_str: Vec<u8>,
    pub is_support: bool, // 委员会是否支持该机器上线
}

impl CommitteeUploadInfo {
    fn join_str<A: ToString>(items: Vec<A>) -> Vec<u8> {
        let mut output = Vec::new();
        for item in items {
            let item: Vec<u8> = item.to_string().into();
            output.extend(item);
        }
        output
    }

    pub fn hash(&self) -> [u8; 16] {
        let is_support: Vec<u8> = if self.is_support { "1".into() } else { "0".into() };

        let mut raw_info = Vec::new();
        raw_info.extend(self.machine_id.clone());
        raw_info.extend(self.gpu_type.clone());
        raw_info.extend(Self::join_str(vec![
            self.gpu_num as u64,
            self.cuda_core as u64,
            self.gpu_mem,
            self.calc_point,
            self.sys_disk,
            self.data_disk,
        ]));
        raw_info.extend(self.cpu_type.clone());
        raw_info.extend(Self::join_str(vec![self.cpu_core_num as u64, self.cpu_rate, self.mem_num]));
        raw_info.extend(self.rand_str.clone());
        raw_info.extend(is_support);

        blake2_128(&raw_info)
    }
}

// 由机器管理者自定义的提交
#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct StakerCustomizeInfo {
    pub server_room: H256,
    /// 上行带宽
    pub upload_net: u64,
    /// 下行带宽
    pub download_net: u64,
    /// 经度(+东经; -西经)
    pub longitude: Longitude,
    /// 纬度(+北纬； -南纬)
    pub latitude: Latitude,
    /// 网络运营商
    pub telecom_operators: Vec<Vec<u8>>,
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Longitude {
    East(u64),
    West(u64),
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum Latitude {
    South(u64),
    North(u64),
}

impl Default for Longitude {
    fn default() -> Self {
        Longitude::East(0)
    }
}

impl Default for Latitude {
    fn default() -> Self {
        Latitude::North(0)
    }
}

/// 记录每个Era的机器的总分
/// NOTE: 这个账户应该是stash账户，而不是controller账户
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct EraStashPoints<AccountId: Ord> {
    /// Total grade of the system (inflation grades from onlineStatus or multipGPU is counted)
    pub total: u64,
    /// Some Era，grade snap of stash account
    pub staker_statistic: BTreeMap<AccountId, StashMachineStatistics>,
}

/// Stash账户的统计
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct StashMachineStatistics {
    /// 用户在线的GPU数量
    pub online_gpu_num: u64,
    /// 用户对应的膨胀系数，由在线GPU数量决定
    pub inflation: Perbill,
    /// 用户的机器的总计算点数得分(不考虑膨胀)
    pub machine_total_calc_point: u64,
    /// 用户机器因被租用获得的额外得分
    pub rent_extra_grade: u64,
}

// 每台机器的基础得分与租用情况
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct MachineGradeStatus {
    /// 机器的基础得分
    pub basic_grade: u64,
    /// 机器的租用状态
    pub is_rented: bool,
}

impl<AccountId> EraStashPoints<AccountId>
where
    AccountId: Ord + Clone,
{
    /// 增加一台在线的机器，gpu数量 + gpu的总得分
    /// NOTE: 只修改当前Era，调用下线逻辑前应检查机器存在
    pub fn change_machine_online_status(&mut self, stash: AccountId, gpu_num: u64, basic_grade: u64, is_online: bool) {
        let mut staker_statistic = self
            .staker_statistic
            .entry(stash.clone())
            .or_insert(StashMachineStatistics { ..Default::default() });

        let old_grade = staker_statistic.total_grades().unwrap();

        if is_online {
            staker_statistic.online_gpu_num = staker_statistic.online_gpu_num.saturating_add(gpu_num);
        } else {
            // 避免上线24小时即下线时，当前Era还没有初始化该值
            staker_statistic.online_gpu_num = staker_statistic.online_gpu_num.checked_sub(gpu_num).unwrap_or_default();
        }

        // 根据显卡数量n更新inflation系数: inflation = min(10%, n/10000)
        // 当stash账户显卡数量n=1000时，inflation最大为10%
        staker_statistic.inflation = if staker_statistic.online_gpu_num <= 1000 {
            Perbill::from_rational_approximation(staker_statistic.online_gpu_num, 10_000)
        } else {
            Perbill::from_rational_approximation(1000u64, 10_000)
        };

        // 根据在线情况更改stash的基础分
        if is_online {
            staker_statistic.machine_total_calc_point =
                staker_statistic.machine_total_calc_point.saturating_add(basic_grade);
        } else {
            staker_statistic.machine_total_calc_point =
                staker_statistic.machine_total_calc_point.checked_sub(basic_grade).unwrap_or_default();
        }

        // 更新系统分数记录
        let new_grade = staker_statistic.total_grades().unwrap();

        self.total = self.total.saturating_add(new_grade).saturating_sub(old_grade);

        // 更新该stash账户的记录
        if staker_statistic.online_gpu_num == 0 {
            self.staker_statistic.remove(&stash);
        } else {
            let staker_statistic = (*staker_statistic).clone();
            self.staker_statistic.insert(stash, staker_statistic);
        }
    }

    /// 因机器租用状态改变，而影响得分
    pub fn change_machine_rent_status(&mut self, stash: AccountId, basic_grade: u64, is_rented: bool) {
        let mut staker_statistic = self
            .staker_statistic
            .entry(stash.clone())
            .or_insert(StashMachineStatistics { ..Default::default() });

        // 因租用而产生的分数
        let grade_by_rent = Perbill::from_rational_approximation(30u64, 100u64) * basic_grade;

        // 更新rent_extra_grade
        if is_rented {
            self.total = self.total.saturating_add(grade_by_rent);
            staker_statistic.rent_extra_grade = staker_statistic.rent_extra_grade.saturating_add(grade_by_rent);
        } else {
            self.total = self.total.saturating_sub(grade_by_rent);
            staker_statistic.rent_extra_grade = staker_statistic.rent_extra_grade.saturating_sub(grade_by_rent);
        }

        let staker_statistic = (*staker_statistic).clone();
        self.staker_statistic.insert(stash, staker_statistic);
    }
}

impl StashMachineStatistics {
    /// 该Stash账户对应的总得分
    /// total_grades = inflation * total_calc_point + total_calc_point + rent_grade
    pub fn total_grades(&self) -> Option<u64> {
        (self.inflation * self.machine_total_calc_point)
            .checked_add(self.machine_total_calc_point)?
            .checked_add(self.rent_extra_grade)
    }
}

impl MachineGradeStatus {
    pub fn machine_actual_grade(&self, inflation: Perbill) -> u64 {
        let rent_extra_grade =
            if self.is_rented { Perbill::from_rational_approximation(30u32, 100u32) * self.basic_grade } else { 0 };
        let inflation_extra_grade = inflation * self.basic_grade;
        self.basic_grade + rent_extra_grade + inflation_extra_grade
    }
}

// 奖励发放前，对所有machine_id进行备份
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct AllMachineIdSnapDetail {
    pub all_machine_id: VecDeque<MachineId>,
    pub snap_len: u64,
}
