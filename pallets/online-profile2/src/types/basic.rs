use crate::{Config, Error};
use codec::{Decode, Encode};
use dbc_support::EraIndex;
use scale_info::TypeInfo;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{traits::Saturating, RuntimeDebug};
use sp_std::vec::Vec;

pub type TelecomName = Vec<u8>;

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
