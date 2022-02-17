use super::super::mock::*;
use frame_support::assert_ok;
use std::convert::TryInto;

// 没人抢单的订单将允许取消
#[test]
fn test_committee_cancel_report_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let reporter: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Eve).into();
        let report_hash: [u8; 16] = hex::decode("986fffc16e63d3f7c43fe1a272ba3ba1").unwrap().try_into().unwrap();
        let reporter_boxpubkey = hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a")
            .unwrap()
            .try_into()
            .unwrap();

        assert_ok!(MaintainCommittee::report_machine_fault(
            Origin::signed(reporter),
            crate::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
        ));

        assert_ok!(MaintainCommittee::reporter_cancel_report(Origin::signed(reporter), 0));
    });
}

// 有人抢单的订单将不允许取消
#[test]
fn test_committee_cancel_booked_report_works() {
    new_test_with_init_params_ext().execute_with(|| {});
}
