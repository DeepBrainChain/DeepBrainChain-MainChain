use super::super::mock::*;
use super::super::Error;
use frame_support::{assert_noop, assert_ok};
use std::convert::TryInto;

// 没人抢单的订单将允许取消
#[test]
fn test_committee_cancel_report_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let reporter = sr25519::Public::from(Sr25519Keyring::Two).into();
        let report_hash: [u8; 16] = hex::decode("986fffc16e63d3f7c43fe1a272ba3ba1").unwrap().try_into().unwrap();
        let reporter_boxpubkey = hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a")
            .unwrap()
            .try_into()
            .unwrap();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();

        // 报告硬件造假允许取消
        {
            assert_ok!(MaintainCommittee::report_machine_fault(
                Origin::signed(reporter),
                crate::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
            ));
            assert_eq!(
                &MaintainCommittee::reporter_stake(reporter),
                &crate::ReporterStakeInfo {
                    staked_amount: 20000 * ONE_DBC,
                    used_stake: 1000 * ONE_DBC,
                    can_claim_reward: 0,
                    claimed_reward: 0
                }
            );

            assert_ok!(MaintainCommittee::reporter_cancel_report(Origin::signed(reporter), 0));
            assert_eq!(
                &MaintainCommittee::reporter_stake(reporter),
                &crate::ReporterStakeInfo {
                    staked_amount: 20000 * ONE_DBC,
                    used_stake: 0,
                    can_claim_reward: 0,
                    claimed_reward: 0
                }
            );
        }

        // 报告租用时无法访问允许取消
        {
            assert_ok!(MaintainCommittee::report_machine_fault(
                Origin::signed(reporter),
                crate::MachineFaultType::RentedInaccessible(machine_id, 0)
            ));
            assert_eq!(
                &MaintainCommittee::reporter_stake(reporter),
                &crate::ReporterStakeInfo {
                    staked_amount: 20000 * ONE_DBC,
                    used_stake: 1000 * ONE_DBC,
                    can_claim_reward: 0,
                    claimed_reward: 0
                }
            );
            assert_ok!(MaintainCommittee::reporter_cancel_report(Origin::signed(reporter), 1));
            assert_eq!(
                &MaintainCommittee::reporter_stake(reporter),
                &crate::ReporterStakeInfo {
                    staked_amount: 20000 * ONE_DBC,
                    used_stake: 0 * ONE_DBC,
                    can_claim_reward: 0,
                    claimed_reward: 0
                }
            );
        }
    });
}

// 有人抢单的订单将不允许取消
#[test]
fn test_committee_cancel_booked_report_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let reporter = sr25519::Public::from(Sr25519Keyring::Two).into();
        let report_hash: [u8; 16] = hex::decode("986fffc16e63d3f7c43fe1a272ba3ba1").unwrap().try_into().unwrap();
        let reporter_boxpubkey = hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a")
            .unwrap()
            .try_into()
            .unwrap();
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48".as_bytes().to_vec();
        let committee = sr25519::Public::from(Sr25519Keyring::One).into();

        // 报告硬件造假允许取消
        {
            assert_ok!(MaintainCommittee::report_machine_fault(
                Origin::signed(reporter),
                crate::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
            ));

            // 委员会订阅机器故障报告
            assert_ok!(MaintainCommittee::committee_book_report(Origin::signed(committee), 0));
            assert_noop!(
                MaintainCommittee::reporter_cancel_report(Origin::signed(reporter), 0),
                Error::<TestRuntime>::OrderNotAllowCancel
            );
        }

        // 报告租用时无法访问允许取消
        {
            assert_ok!(MaintainCommittee::report_machine_fault(
                Origin::signed(reporter),
                crate::MachineFaultType::RentedInaccessible(machine_id, 0)
            ));

            // 委员会订阅机器故障报告
            assert_ok!(MaintainCommittee::committee_book_report(Origin::signed(committee), 1));

            assert_noop!(
                MaintainCommittee::reporter_cancel_report(Origin::signed(reporter), 1),
                Error::<TestRuntime>::OrderNotAllowCancel
            );
        }
    });
}
