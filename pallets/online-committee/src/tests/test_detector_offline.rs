// [Thread B ① · DLC 化] DDN 检测器免租举报离线路径单测。
use crate::mock::*;
use dbc_support::machine_type::MachineStatus;
use frame_support::{assert_noop, assert_ok};
use sp_core::sr25519;
use sp_keyring::sr25519::Keyring as Sr25519Keyring;

fn machine_id() -> Vec<u8> {
    "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
        .as_bytes()
        .to_vec()
}
fn detector() -> sr25519::Public {
    sr25519::Public::from(Sr25519Keyring::One)
}
fn stranger() -> sr25519::Public {
    sr25519::Public::from(Sr25519Keyring::Two)
}

// 授权检测器可对在线机器上报离线（不要求被租），机器进入离线状态
#[test]
fn detector_can_report_offline() {
    new_test_with_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::set_offline_detectors(RuntimeOrigin::root(), vec![detector()]));
        assert!(OnlineProfile::offline_detectors().contains(&detector()));

        assert_ok!(OnlineProfile::report_machine_offline_by_detector(
            RuntimeOrigin::signed(detector()),
            machine_id()
        ));
        let mi = OnlineProfile::machines_info(&machine_id()).unwrap();
        assert!(
            matches!(mi.machine_status, MachineStatus::StakerReportOffline(..)),
            "machine should be offline after detector report, got {:?}",
            mi.machine_status
        );
    })
}

// 非检测器调用被拒（fail-closed）
#[test]
fn non_detector_rejected() {
    new_test_with_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::set_offline_detectors(RuntimeOrigin::root(), vec![detector()]));
        assert_noop!(
            OnlineProfile::report_machine_offline_by_detector(
                RuntimeOrigin::signed(stranger()),
                machine_id()
            ),
            online_profile::Error::<TestRuntime>::NotOfflineDetector
        );
        // 空集时连被授权前的检测器也不行（fail-closed 基线）
        assert_ok!(OnlineProfile::set_offline_detectors(RuntimeOrigin::root(), vec![]));
        assert_noop!(
            OnlineProfile::report_machine_offline_by_detector(
                RuntimeOrigin::signed(detector()),
                machine_id()
            ),
            online_profile::Error::<TestRuntime>::NotOfflineDetector
        );
    })
}

// 已离线机器再报被拒（幂等保护）
#[test]
fn report_rejected_when_already_offline() {
    new_test_with_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::set_offline_detectors(RuntimeOrigin::root(), vec![detector()]));
        assert_ok!(OnlineProfile::report_machine_offline_by_detector(
            RuntimeOrigin::signed(detector()),
            machine_id()
        ));
        assert_noop!(
            OnlineProfile::report_machine_offline_by_detector(
                RuntimeOrigin::signed(detector()),
                machine_id()
            ),
            online_profile::Error::<TestRuntime>::MachineStatusNotAllowed
        );
    })
}

// set_offline_detectors 仅 root
#[test]
fn set_offline_detectors_requires_root() {
    new_test_with_machine_online().execute_with(|| {
        assert_noop!(
            OnlineProfile::set_offline_detectors(
                RuntimeOrigin::signed(detector()),
                vec![detector()]
            ),
            sp_runtime::DispatchError::BadOrigin
        );
    })
}
