#![cfg_attr(not(feature = "std"), no_std)]

use node_primitives::Balance;

// TODO: how to get balance type???
pub trait PhaseReward {
    fn set_phase0_reward(balance: Balance) -> u64; // TODO: add balance type
    fn set_phase1_reward(balance: Balance) -> u64;
    fn set_phase2_reward(balance: Balance) -> u64;
}
