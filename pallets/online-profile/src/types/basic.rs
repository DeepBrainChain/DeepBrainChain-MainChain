use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{traits::Saturating, RuntimeDebug};
use sp_std::vec::Vec;

pub type EraIndex = u32;
pub type TelecomName = Vec<u8>;

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

/// Standard GPU rent price Per Era
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct StandardGpuPointPrice {
    /// Standard GPU calc points
    pub gpu_point: u64,
    /// Standard GPU price
    pub gpu_price: u64,
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

    // NOTE: 与下线不同，退出时，不增加offline_gpu数量
    // 返回是否为空
    pub fn machine_exit(&mut self, gpu_num: u32, calc_point: u64) -> bool {
        self.online_gpu = self.online_gpu.saturating_sub(gpu_num as u64);
        self.online_gpu_calc_points = self.online_gpu_calc_points.saturating_sub(calc_point);
        self == &PosInfo::default()
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
