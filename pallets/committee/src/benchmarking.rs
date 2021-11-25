#![cfg(feature = "runtime-benchmarks")]

pub use frame_benchmarking::{account, benchmarks};
use frame_system::RawOrigin;

use super::*;

const SEED: u32 = 0;

benchmarks! {
    add_committee {
        let u in 0..1000;
        let user: T::AccountId = account("user", 0, SEED);
    }: _ (RawOrigin::Root, user.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{Balances, TestRuntime};
    use frame_support::assert_ok;

    #[test]
    fn test_benchmarks() {
        // ExtBuilder::default().build().execute_with(|| {
        crate::mock::new_test_with_init_params_ext().execute_with(|| {
            assert_ok!(test_benchmark_add_committee::<TestRuntime>());
        })
    }
}
