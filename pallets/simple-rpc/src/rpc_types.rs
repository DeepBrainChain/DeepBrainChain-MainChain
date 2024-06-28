#[cfg(feature = "std")]
use dbc_support::rpc_types::serde_text;
use parity_scale_codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::vec::Vec;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Debug, Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct StakerListInfo<Balance, AccountId> {
    pub index: u64,
    #[cfg_attr(feature = "std", serde(with = "serde_text"))]
    pub staker_name: Vec<u8>,
    pub staker_account: AccountId,
    pub calc_points: u64,
    pub total_gpu_num: u64,
    pub total_rented_gpu: u64,
    pub total_rent_fee: Balance, // 总租金收益(银河竞赛前获得)
    pub total_burn_fee: Balance, // 总销毁数量
    pub total_reward: Balance,
    pub total_released_reward: Balance,
}
