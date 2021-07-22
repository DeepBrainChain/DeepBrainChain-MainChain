use crate::mock::*;
use frame_support::assert_ok;
use std::convert::TryInto;

#[test]
fn report_machine_fault_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let committee: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::One).into();
        let reporter: sp_core::sr25519::Public = sr25519::Public::from(Sr25519Keyring::Two).into();

        let report_hash: [u8; 16] =
            hex::decode("986fffc16e63d3f7c43fe1a272ba3ba1").unwrap().try_into().unwrap();

        let machine_id =
            "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
        let reporter_rand_str = "abcdef".as_bytes().to_vec();
        let committee_rand_str = "fedcba".as_bytes().to_vec();
        let err_reason = "它坏了".as_bytes().to_vec();
        let committee_hash =
            hex::decode("0029f96394d458279bcd0c232365932a").unwrap().try_into().unwrap();

        let reporter_boxpubkey =
            hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a")
                .unwrap()
                .try_into()
                .unwrap();
        assert_ok!(MaintainCommittee::report_machine_fault(
            Origin::signed(reporter),
            report_hash,
            reporter_boxpubkey
        ));

        let mut report_status = crate::MTReportInfoDetail {
            reporter,
            report_time: 11,
            reporter_stake: 1250 * ONE_DBC, // 15,000,000 / 12,000
            machine_fault_type: crate::MachineFaultType::HardwareFault,
            reporter_boxpubkey,
            reporter_hash: report_hash,
            ..Default::default()
        };

        assert_eq!(&MaintainCommittee::report_info(0), &report_status);

        assert_ok!(MaintainCommittee::book_fault_order(Origin::signed(committee), 0));

        // TODO: 提交加密信息
        let encrypted_err_info = hex::decode("").unwrap().try_into().unwrap();
        assert_ok!(MaintainCommittee::reporter_add_encrypted_error_info(
            Origin::signed(reporter),
            0,
            committee,
            encrypted_err_info
        ));

        report_status.first_book_time = 11;
        report_status.verifying_committee = Some(committee);
        report_status.booked_committee.push(committee);
        report_status.get_encrypted_info_committee.push(committee);
        report_status.report_status = crate::ReportStatus::Verifying;
        report_status.confirm_start = 11 + 360;
        assert_eq!(&MaintainCommittee::report_info(0), &report_status);

        // 提交验证Hash
        assert_ok!(MaintainCommittee::submit_confirm_hash(
            Origin::signed(committee),
            0,
            committee_hash
        ));

        report_status.verifying_committee = None;
        report_status.hashed_committee.push(committee);
        report_status.report_status = crate::ReportStatus::WaitingBook;
        assert_eq!(&MaintainCommittee::report_info(0), &report_status);

        // 3个小时之后才能提交：
        run_to_block(360 + 13);

        report_status.report_status = crate::ReportStatus::SubmittingRaw;
        assert_eq!(&MaintainCommittee::report_info(0), &report_status);

        assert_eq!(&MaintainCommittee::report_info(0), &report_status);

        assert_ok!(MaintainCommittee::submit_confirm_raw(
            Origin::signed(committee),
            0,
            machine_id,
            reporter_rand_str,
            committee_rand_str,
            err_reason,
            true
        ));
    })
}

#[test]
fn report_machine_offline_works() {}

#[test]
fn report_machine_unrentable_works() {}

// 控制账户报告机器下线
#[test]
fn controller_report_online_machine_offline_should_work() {
    new_test_with_online_machine_online_ext().execute_with(|| {})
}
