use super::super::{mock::*, ReporterStakeInfo};
use dbc_support::verify_slash::{OPPendingSlashInfo, OPSlashReason};
use frame_support::assert_ok;
use once_cell::sync::Lazy;
use std::convert::TryInto;
use dbc_support::ONE_DAY;

const controller: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Eve));

const committee1: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::One));
const committee2: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Two));
const committee3: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Ferdie));

const reporter: Lazy<sp_core::sr25519::Public> = committee2;

// 报告其他类型的错误
#[test]
fn report_machine_fault_fulfilling_works() {
    new_test_with_init_params_ext().execute_with(|| {
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);
        let reporter_boxpubkey =
            hex::decode("1e71b5a83ccdeff1592062a1d4da4a272691f08e2024a1ca75a81d534a76210a")
                .unwrap()
                .try_into()
                .unwrap();
        let report_hash: [u8; 16] =
            hex::decode("2611557f5306f050019eeb27648c5494").unwrap().try_into().unwrap();

        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();
        let reporter_rand_str = "abcdef".as_bytes().to_vec();
        let committee_rand_str = "abc1".as_bytes().to_vec();
        let err_reason = "补充信息，可留空".as_bytes().to_vec();
        let committee_hash: [u8; 16] =
            hex::decode("7980cfd18a2e6cb338f4924ae0fff495").unwrap().try_into().unwrap();

        assert_ok!(MaintainCommittee::report_machine_fault(
            RuntimeOrigin::signed(*reporter),
            crate::MachineFaultType::RentedHardwareMalfunction(report_hash, reporter_boxpubkey),
        ));
        // 委员会订阅机器故障报告
        assert_ok!(MaintainCommittee::committee_book_report(RuntimeOrigin::signed(*committee1), 0));

        // 提交加密信息
        let encrypted_err_info: Vec<u8> =
            hex::decode("01405deeef2a8b0f4a09380d14431dd10fde1ad62b3c27b3fbea4701311d")
                .unwrap()
                .try_into()
                .unwrap();
        assert_ok!(MaintainCommittee::reporter_add_encrypted_error_info(
            RuntimeOrigin::signed(*reporter),
            0,
            *committee1,
            encrypted_err_info.clone()
        ));
        // 提交验证Hash
        assert_ok!(MaintainCommittee::committee_submit_verify_hash(
            RuntimeOrigin::signed(*committee1),
            0,
            committee_hash.clone()
        ));

        // 3个小时之后才能提交：
        run_to_block(360 + 13);

        // submit_confirm_raw:
        // - Writes:
        // ReportInfo, CommitteeOps
        let extra_err_info = Vec::new();
        assert_ok!(MaintainCommittee::committee_submit_verify_raw(
            RuntimeOrigin::signed(*committee1),
            0,
            machine_id.clone(),
            0,
            reporter_rand_str,
            committee_rand_str,
            err_reason.clone(),
            extra_err_info,
            true
        ));

        run_to_block(360 + 14);

        run_to_block(ONE_DAY * 2 + 374);
        {
            assert_eq!(
                MaintainCommittee::reporter_stake(&*reporter),
                crate::ReporterStakeInfo { staked_amount: 20000 * ONE_DBC, ..Default::default() }
            );
            assert_eq!(Committee::committee_stake(*committee1).used_stake, 0);
            assert_eq!(Committee::committee_stake(*committee1).staked_amount, 20000 * ONE_DBC);
        }

        // 报告人上线机器
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));
        {
            assert_eq!(OnlineProfile::pending_exec_slash(ONE_DAY * 4 + 375), vec![0],);
            assert_eq!(
                OnlineProfile::pending_slash(0),
                OPPendingSlashInfo {
                    slash_who: stash,
                    machine_id,
                    slash_time: ONE_DAY * 2 + 375,
                    slash_amount: 240000 * ONE_DBC,
                    slash_exec_time: ONE_DAY * 4 + 375,
                    reporter: Some(*reporter),
                    renters: vec![],
                    reward_to_committee: Some(vec![*committee1]),
                    slash_reason: OPSlashReason::RentedHardwareMalfunction(11),
                }
            );
        }
        run_to_block(ONE_DAY * 4 + 376);
    })
}
