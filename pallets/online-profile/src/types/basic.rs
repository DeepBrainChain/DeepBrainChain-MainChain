use crate::{Config, Error};
use codec::{Decode, Encode};
use dbc_support::EraIndex;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{traits::Saturating, RuntimeDebug};
use sp_std::vec::Vec;

pub type TelecomName = Vec<u8>;

/// Reward duration for committee (Era)
pub const REWARD_DURATION: u32 = 365 * 2;
/// Rebond frequency, 1 year
pub const REBOND_FREQUENCY: u32 = 365 * 2880;
/// Max Slash Threshold: 120h, 5 era
pub const MAX_SLASH_THRESHOLD: u32 = 2880 * 5;
// PendingSlash will be exec in two days

use dbc_support::custom_err::OnlineErr;
impl<T: Config> From<OnlineErr> for Error<T> {
    fn from(err: OnlineErr) -> Self {
        match err {
            OnlineErr::ClaimRewardFailed => Error::ClaimRewardFailed,
            OnlineErr::NotMachineController => Error::NotMachineController,
            OnlineErr::CalcStakeAmountFailed => Error::CalcStakeAmountFailed,
            OnlineErr::TelecomIsNull => Error::TelecomIsNull,
            OnlineErr::NotAllowedChangeMachineInfo => Error::NotAllowedChangeMachineInfo,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct UserMutHardwareStakeInfo<Balance, BlockNumber> {
    pub verify_fee: Balance,    // 支付给审核人
    pub offline_slash: Balance, // 下线惩罚
    pub offline_time: BlockNumber,
    pub need_fulfilling: bool, // 记录是否需要补交质押
}

// 365 day per year
// Testnet start from 2021-07-18, after 3 years(365*3), in 2024-07-17, phase 1 should end.
// If galxy is on, Reward is double in 60 eras. So, phase 1 should end in 2024-05-18 (365*3-60)
// So, **first_phase_duration** should equal: 365 * 3 - 60 - (online_day - 2021-0718)
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
pub struct PhaseRewardInfoDetail<Balance> {
    pub online_reward_start_era: EraIndex, // When online reward will start
    pub first_phase_duration: EraIndex,
    pub galaxy_on_era: EraIndex, // When galaxy is on (开启后100%销毁租金，此后60天奖励翻倍)
    pub phase_0_reward_per_era: Balance, // first 3 years
    pub phase_1_reward_per_era: Balance, // next 5 years
    pub phase_2_reward_per_era: Balance, // next 5 years
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
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
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
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
    pub fn on_stake_changed(&mut self, amount: Balance, is_add: bool) {
        if is_add {
            self.total_stake = self.total_stake.saturating_add(amount);
        } else {
            self.total_stake = self.total_stake.saturating_sub(amount);
        }
    }

    pub fn change_rent_fee(&mut self, fee_to_destroy: Balance, fee_to_stash: Balance) {
        self.total_burn_fee = self.total_burn_fee.saturating_add(fee_to_destroy);
        self.total_rent_fee = self.total_rent_fee.saturating_add(fee_to_stash);
    }

    pub fn on_rent_fee_changed(&mut self, rent_fee: Balance, burn_fee: Balance) {
        self.total_rent_fee = self.total_rent_fee.saturating_add(rent_fee);
        self.total_burn_fee = self.total_burn_fee.saturating_add(burn_fee);
    }
}

/// Statistics of gpus based on position(latitude and longitude)
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug, TypeInfo)]
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
    pub fn on_rent_changed(&mut self, is_rented: bool, gpu_num: u32) {
        if is_rented {
            self.rented_gpu = self.rented_gpu.saturating_add(gpu_num as u64);
        } else {
            self.rented_gpu = self.rented_gpu.saturating_sub(gpu_num as u64);
        }
    }

    pub fn on_online_changed(&mut self, is_online: bool, gpu_num: u32, calc_point: u64) {
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
    pub fn on_machine_exit(&mut self, gpu_num: u32, calc_point: u64) -> bool {
        self.online_gpu = self.online_gpu.saturating_sub(gpu_num as u64);
        self.online_gpu_calc_points = self.online_gpu_calc_points.saturating_sub(calc_point);
        self == &PosInfo::default()
    }
}
