use crate::mock::*;
use frame_support::assert_ok;
use online_profile::{Phase1Destruction, Phase2Destruction};
use sp_runtime::Perbill;

#[test]
fn test_phase1_destruction_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        Phase1Destruction::<TestRuntime>::put((
            2500,
            Perbill::from_rational_approximation(50u32, 100u32),
            true,
        ));
        Phase2Destruction::<TestRuntime>::put((
            5000,
            Perbill::from_rational_approximation(100u32, 100u32),
            false,
        ));

        // 开始租用
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let renter1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        // Committee2 is also Two
        let pot_two = sr25519::Public::from(Sr25519Keyring::Two);

        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(renter1),
            machine_id.clone(),
            4,
            60
        ));
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(renter1), 0));

        // 检查租金
        assert_eq!(
            Balances::free_balance(&stash),
            INIT_BALANCE - 400000 * ONE_DBC + 259939208333333333
        );
        assert_eq!(
            Balances::free_balance(&pot_two),
            259939208333333333 + INIT_BALANCE - 20000 * ONE_DBC
        );
    })
}

#[test]
fn test_phase2_destruction_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        Phase1Destruction::<TestRuntime>::put((
            2500,
            Perbill::from_rational_approximation(50u32, 100u32),
            true,
        ));
        Phase2Destruction::<TestRuntime>::put((
            5000,
            Perbill::from_rational_approximation(100u32, 100u32),
            true,
        ));

        // 开始租用
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let renter1 = sr25519::Public::from(Sr25519Keyring::Alice);
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        // Committee2 is also Two
        let pot_two = sr25519::Public::from(Sr25519Keyring::Two);

        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(renter1),
            machine_id.clone(),
            4,
            60
        ));
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(renter1), 0));

        // 检查租金
        assert_eq!(Balances::free_balance(&stash), INIT_BALANCE - 400000 * ONE_DBC);
        assert_eq!(
            Balances::free_balance(&pot_two),
            259939208333333333 * 2 + INIT_BALANCE - 20000 * ONE_DBC
        );
    })
}
