// This file is part of Substrate.

// Copyright (C) 2019-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! This module expose one function `P_NPoS` (Payout NPoS) or `compute_total_payout` which returns
//! the total payout for the era given the era duration and the staking rate in NPoS.
//! The staking rate in NPoS is the total amount of tokens staked by nominators and validators,
//! divided by the total token supply.

use sp_runtime::{traits::AtLeast32BitUnsigned, Perbill};

/// The total payout to all validators (and their nominators) per era and maximum payout.
///
/// Defined as such:
/// `staker-payout = yearly_inflation(npos_token_staked / total_tokens) * total_tokens / era_per_year`
/// `maximum-payout = max_yearly_inflation * total_tokens / era_per_year`
///
/// `era_duration` is expressed in millisecond.
pub fn compute_total_payout<N>(
    milliseconds_per_year: u64,
    yearly_inflation_amount: N,
    era_duration: u64,
) -> (N, N)
where
    N: AtLeast32BitUnsigned + Clone,
{
    let portion = Perbill::from_rational_approximation(era_duration as u64, milliseconds_per_year);
    let payout = portion * yearly_inflation_amount;

    // 每年通胀的量，全部发放给节点，因此max=inflation_payout
    (payout.clone(), payout)
}

#[cfg(test)]
mod test {
    use sp_runtime::curve::PiecewiseLinear;

    pallet_staking_reward_curve::build! {
        const I_NPOS: PiecewiseLinear<'static> = curve!(
            min_inflation: 0_025_000,
            max_inflation: 0_100_000,
            ideal_stake: 0_500_000,
            falloff: 0_050_000,
            max_piece_count: 40,
            test_precision: 0_005_000,
        );
    }

    #[test]
    fn npos_curve_is_sensible() {
        const YEAR: u64 = 36525 * 24 * 60 * 60 * 1000 / 100;
        let era_duration: u64 = 365 * 24 * 60 * 60 * 1000;

        // check maximum inflation.
        // not 10_000 due to rounding error.
        assert_eq!(super::compute_total_payout(YEAR, 1_000_000_000u64, era_duration).1, 999_315_537); // 最大值为 total_token * 10%, 这个10% 由I_NPOS 中的max_inflation所定义

        //super::I_NPOS.calculate_for_fraction_times_denominator(25, 100)
        assert_eq!(super::compute_total_payout(YEAR, 0u64, era_duration).0, 0);
        assert_eq!(super::compute_total_payout(YEAR, 5_000u64, era_duration).0, 4_997);
        assert_eq!(super::compute_total_payout(YEAR, 25_000u64, era_duration).0, 24_983);
        assert_eq!(super::compute_total_payout(YEAR, 40_000u64, era_duration).0, 39_973);
        assert_eq!(super::compute_total_payout(YEAR, 50_000u64, era_duration).0, 49_966);
        assert_eq!(super::compute_total_payout(YEAR, 60_000u64, era_duration ).0, 59_959);
        assert_eq!(super::compute_total_payout(YEAR, 75_000u64, era_duration).0, 74_949);
        assert_eq!(super::compute_total_payout(YEAR, 95_000u64, era_duration).0, 94_935);
        assert_eq!(super::compute_total_payout(YEAR, 100_000u64, era_duration).0, 99_932);

        const DAY: u64 = 24 * 60 * 60 * 1000;
        assert_eq!(super::compute_total_payout(YEAR, 25_000u64, DAY).0, 68);
        assert_eq!(super::compute_total_payout(YEAR, 50_000u64, DAY).0, 137);
        assert_eq!(super::compute_total_payout(YEAR, 75_000u64, DAY).0, 205);

        const SIX_HOURS: u64 = 6 * 60 * 60 * 1000;
        assert_eq!(super::compute_total_payout(DAY, 25_000u64, SIX_HOURS).0, 6_250);
        assert_eq!(super::compute_total_payout(DAY, 50_000u64, SIX_HOURS).0, 12_500);
        assert_eq!(super::compute_total_payout(DAY, 75_000u64, SIX_HOURS).0, 18_750);

        const HOUR: u64 = 60 * 60 * 1000;
        assert_eq!(super::compute_total_payout(SIX_HOURS, 2_500_000_000_000u64, HOUR).0,
                   416_666_665_000
        );
    }
}
