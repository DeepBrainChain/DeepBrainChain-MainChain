use crate::{mock::*, Error};

use dbc_support::{rental_type::RentStatus, traits::DLCMachineReportStakingTrait, ONE_DAY};
use frame_support::{assert_err, assert_ok};
use once_cell::sync::Lazy;

use rent_machine::Error as RentMachineError;
use sp_core::Pair;
use sp_keyring::AccountKeyring::{Dave, Eve};
const renter_dave: Lazy<sr25519::Public> = Lazy::new(|| sr25519::Public::from(Dave));

const renter_owner: Lazy<sr25519::Public> = Lazy::new(|| sr25519::Public::from(Eve));
const machine_id: Lazy<Vec<u8>> = Lazy::new(|| {
    "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
        .as_bytes()
        .to_vec()
});

#[test]
fn report_dlc_staking_should_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        let msg: Vec<u8> = b"abc".to_vec();
        let eve_sig = Eve.sign(&msg[..]);
        assert_eq!(Eve.public(), sr25519::Public::from(Sr25519Keyring::Eve));

        // 过10个块之后执行租用成功
        run_to_block(10 + 20);

        assert_err!(
            <dlc_machine::Pallet<TestRuntime> as DLCMachineReportStakingTrait>::report_dlc_staking(
                msg.clone(),
                eve_sig.clone(),
                Eve.public(),
                machine_id.clone()
            ),
            RentMachineError::<TestRuntime>::MachineNotRented
        );

        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_owner),
            machine_id.clone(),
            4,
            (10 * ONE_DAY) as u64
        ));

        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_owner), 0));

        assert_eq!(
            dlc_machine::DLCMachinesInStaking::<TestRuntime>::contains_key(machine_id.clone()),
            false
        );

        assert_ok!(
            <dlc_machine::Pallet<TestRuntime> as DLCMachineReportStakingTrait>::report_dlc_staking(
                msg,
                eve_sig,
                Eve.public(),
                machine_id.clone()
            )
        );
        assert_eq!(
            dlc_machine::DLCMachinesInStaking::<TestRuntime>::contains_key(machine_id.clone()),
            true
        )
    })
}

#[test]
fn rent_dlc_machine_should_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        let eve = sp_core::sr25519::Pair::from(Eve);
        let msg: Vec<u8> = b"abc".to_vec();
        let eve_sig = eve.sign(&msg[..]);
        assert_eq!(eve.public(), sr25519::Public::from(Eve));

        // 过10个块之后执行租用成功
        run_to_block(10 + 20);

        assert_err!(
            <dlc_machine::Pallet<TestRuntime> as DLCMachineReportStakingTrait>::report_dlc_staking(
                msg.clone(),
                eve_sig.clone(),
                eve.public(),
                machine_id.clone()
            ),
            RentMachineError::<TestRuntime>::MachineNotRented
        );

        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_owner),
            machine_id.clone(),
            4,
            (10 * ONE_DAY) as u64
        ));

        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_owner), 0));
        assert_eq!(
            dlc_machine::DLCMachinesInStaking::<TestRuntime>::contains_key(machine_id.clone()),
            false
        );
        assert_err!(
            RentDlcMachine::rent_dlc_machine(
                RuntimeOrigin::signed(*renter_dave),
                machine_id.clone(),
                4,
                (10 * ONE_DAY * 2) as u64
            ),
            Error::<TestRuntime>::MachineNotDLCStaking
        );

        assert_ok!(
            <dlc_machine::Pallet<TestRuntime> as DLCMachineReportStakingTrait>::report_dlc_staking(
                msg,
                eve_sig,
                eve.public(),
                machine_id.clone()
            )
        );
        assert_eq!(
            dlc_machine::DLCMachinesInStaking::<TestRuntime>::contains_key(machine_id.clone()),
            true
        );

        // renter's dlc balance should be 10000000*ONE_DLC before rent dlc machine
        let asset_id = RentDlcMachine::get_dlc_asset_id_parameter();
        assert_eq!(Assets::balance(asset_id.into(), *renter_dave), 10000000 * ONE_DLC);

        //  dlc total_supply should be 10000000*ONE_DLC before rent dlc machine
        assert_eq!(Assets::total_supply(asset_id.into()), 10000000 * ONE_DLC);

        assert_ok!(RentDlcMachine::rent_dlc_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            (10 * ONE_DAY * 2) as u64
        ));

        let burn_total = RentDlcMachine::burn_total_amount();
        assert_ne!(burn_total, 0);
        // renter's dlc balance should less than 10000000*ONE_DLC after rent dlc machine
        assert_eq!(Assets::balance(asset_id.into(), *renter_dave), 10000000 * ONE_DLC - burn_total);

        //  dlc total_supply should less than 10000000*ONE_DLC after rent dlc machine
        assert_eq!(Assets::total_supply(asset_id.into()), 10000000 * ONE_DLC - burn_total);

        let rent_order_infos = RentDlcMachine::machine_rent_order(machine_id.clone());
        assert_eq!(rent_order_infos.used_gpu.len(), 4);
        assert_eq!(rent_order_infos.rent_order.len(), 1);
        let rent_dlc_machine_id = rent_order_infos.rent_order[0];
        let rent_info = RentDlcMachine::rent_info(rent_dlc_machine_id.clone()).unwrap();
        assert_eq!(rent_info.rent_status, RentStatus::Renting);
        let rent_duration = rent_info.rent_end.saturating_sub(rent_info.rent_start);
        assert_eq!(rent_duration, 10 * ONE_DAY as u64);

        let records = RentDlcMachine::burn_records();
        assert_eq!(records.len(), 1);
        let (burn_amount, burn_at, renter, rent_id) = records[0];
        assert_eq!(burn_amount, burn_total);
        assert_ne!(burn_at, 0);
        assert_eq!(rent_id, rent_dlc_machine_id);
        assert_eq!(renter, Dave.public());

        let dbc_rent_order_infos = RentMachine::machine_rent_order(machine_id.clone());
        let rent_dbc_machine_id = dbc_rent_order_infos.rent_order[0];
        assert_eq!(
            RentDlcMachine::dlc_rent_id_2_parent_dbc_rent_id(rent_dlc_machine_id.clone()).unwrap(),
            rent_dbc_machine_id
        );

        let rent_ids = RentDlcMachine::user_order(Dave.public());
        assert_eq!(rent_ids.len(), 1);

        let rented_gpu_num = RentDlcMachine::dlc_machine_rented_gpu(machine_id.clone());
        assert_eq!(rented_gpu_num, 4);

        run_to_block((30 + ONE_DAY * 10 + 1) as BlockNumber);

        let rent_ids = RentDlcMachine::user_order(Dave.public());
        assert_eq!(rent_ids.len(), 0);

        assert_eq!(RentDlcMachine::rent_info(rent_dlc_machine_id.clone()).is_none(), true);

        let dlc_rent_order_infos = RentDlcMachine::machine_rent_order(machine_id.clone());
        assert_eq!(dlc_rent_order_infos.rent_order.len(), 0);
        assert_eq!(dlc_rent_order_infos.used_gpu.len(), 0);

        assert_eq!(
            RentDlcMachine::dlc_rent_id_2_parent_dbc_rent_id(rent_dlc_machine_id.clone()).unwrap(),
            rent_dbc_machine_id
        );

        let rented_gpu_num = RentDlcMachine::dlc_machine_rented_gpu(machine_id.clone());
        assert_eq!(rented_gpu_num, 0);
    })
}
