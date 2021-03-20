#![cfg_attr(not(feature = "std"), no_std)]

pub trait PhaseReward {
    type Balance;

    fn set_phase0_reward(balance: Self::Balance);
    fn set_phase1_reward(balance: Self::Balance);
    fn set_phase2_reward(balance: Self::Balance);
}
