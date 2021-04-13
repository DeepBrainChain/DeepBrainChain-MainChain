use sp_runtime::{traits::AtLeast32BitUnsigned, Perbill};

/// grade_percentage = 1 / (1 + 1 / x^4)
/// x = staked_in / machine_price
pub fn compute_stake_grades<N>(machine_price: N, staked_in: N, machine_grade: u64) -> u64
where
    N: AtLeast32BitUnsigned + Clone,
{
    if staked_in >= machine_price {
        let x = Perbill::from_rational_approximation(machine_price, staked_in);
        let x4 = x.square().square();

        let percentage = Perbill::from_rational_approximation(1000000, 1000000u64 + x4 * 1000000);
        return percentage * machine_grade;
    }

    // 否则: machine_price > staked_in {
    let x = Perbill::from_rational_approximation(staked_in, machine_price);
    let x4 = x.square().square();

    let percentage = Perbill::from_rational_approximation(x4 * 1000000u64, x4 * 1000000 + 1000000);
    return percentage * machine_grade;
}

#[cfg(test)]
mod test {
    #[test]
    #[rustfmt::skip]
    fn grade_curve_is_sensible() {

        let machine_price: u64 = 10_000;

        assert_eq!(super::compute_stake_grades(machine_price, 1_000u64, 800u64), 0u64);
        assert_eq!(super::compute_stake_grades(machine_price, 2_000u64, 800u64), 1u64);
        assert_eq!(super::compute_stake_grades(machine_price, 5_000u64, 800u64), 47u64);

        assert_eq!(super::compute_stake_grades(machine_price, 10_000u64, 800u64), 400u64);
        assert_eq!(super::compute_stake_grades(machine_price, 15_000u64, 800u64), 668u64);
        assert_eq!(super::compute_stake_grades(machine_price, 20_000u64, 800u64), 753u64);
        assert_eq!(super::compute_stake_grades(machine_price, 25_000u64, 800u64), 780u64);
        assert_eq!(super::compute_stake_grades(machine_price, 30_000u64, 800u64), 790u64);
    }
}
