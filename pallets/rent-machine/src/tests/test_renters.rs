use super::super::mock::*;
use dbc_support::{
    machine_type::MachineStatus,
    rental_type::{RentOrderDetail, RentStatus},
};
use frame_support::assert_ok;

#[test]
fn test_renters_change_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        let renter1 = sr25519::Public::from(Sr25519Keyring::Alice);

        // 对四卡的机器分两次租用
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter1), machine_id.clone(), 2, 60));
        assert_ok!(RentMachine::rent_machine(Origin::signed(renter1), machine_id.clone(), 2, 120));

        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter1), 0));
        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter1), 1));

        let machine_info = OnlineProfile::machines_info(&machine_id);
        assert_eq!(machine_info.renters, vec![renter1]);

        assert_eq!(
            RentMachine::rent_info(0),
            RentOrderDetail {
                machine_id: machine_id.clone(),
                renter: renter1,
                rent_start: 11,
                confirm_rent: 11,
                rent_end: 11 + 60,
                stake_amount: 0,
                rent_status: RentStatus::Renting,
                gpu_num: 2,
                gpu_index: vec![0, 1],
            }
        );

        run_to_block(80);

        let machine_info = OnlineProfile::machines_info(&machine_id);
        assert_eq!(machine_info.machine_status, MachineStatus::Rented);
        assert_eq!(machine_info.renters, vec![renter1]);

        run_to_block(140);

        let machine_info = OnlineProfile::machines_info(&machine_id);
        assert!(machine_info.renters.is_empty());
    })
}

#[test]
fn test_renters_change_works2() {
    new_test_ext_after_machine_online().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        let renter1 = sr25519::Public::from(Sr25519Keyring::Alice);

        assert_ok!(RentMachine::rent_machine(Origin::signed(renter1), machine_id.clone(), 2, 60));
        assert_ok!(RentMachine::confirm_rent(Origin::signed(renter1), 0));

        let machine_info = OnlineProfile::machines_info(&machine_id);
        assert_eq!(machine_info.renters, vec![renter1]);

        run_to_block(80);

        let machine_info = OnlineProfile::machines_info(&machine_id);
        assert_eq!(machine_info.machine_status, MachineStatus::Online);
        assert!(machine_info.renters.is_empty());
    })
}
