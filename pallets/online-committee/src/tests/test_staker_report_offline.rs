use crate::{
    mock::*,
    tests::{controller, stash},
};
use dbc_support::{
    live_machine::LiveMachine,
    ONE_DAY, ONE_MINUTE,
};
use frame_support::assert_ok;

// 2). 机器在空闲状态
//
// [Thread B ② · DLC 化 2026-07-02] 闲置（未被租）机器离线**不再惩罚质押**（feng: "不罚了"）。
//   奖励仍然自动停发（机器离线时从 era 点数快照移除 → 0 奖励，重新上线自动恢复），但质押 bond 保持不动，
//   允许闲置机器随时上下线。原来的 2%/4%/30%/80% 分档全部归 0（slash_percent(OnlineReportOffline)=0）。
//   下面四个用例覆盖原四个时长档(≤7min / ≤48h / ≤10天 / >10天)，现在**全部断言零 slash、质押不变**。
//   租用中离线不受影响（租金惩罚由 ③ 托管改造处理）。

// helper: 闲置机器下线→（duration 后）上线，断言零惩罚、质押不变、机器恢复在线
fn assert_idle_offline_no_slash(run_to: u32) {
    let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
        .as_bytes()
        .to_vec();

    assert_ok!(OnlineProfile::controller_report_offline(
        RuntimeOrigin::signed(*controller),
        machine_id.clone()
    ));

    run_to_block(run_to);
    assert_ok!(OnlineProfile::controller_report_online(
        RuntimeOrigin::signed(*controller),
        machine_id.clone()
    ));

    // [②] 闲置离线不产生任何 slash 记录
    assert_eq!(OnlineProfile::pending_slash(0), None, "idle offline must NOT slash (Thread B ②)");
    assert_eq!(OnlineProfile::pending_slash(1), None);

    // 质押 bond 全程不动
    assert_eq!(Balances::reserved_balance(*stash), 40000 * ONE_DBC);
    assert_eq!(OnlineProfile::stash_stake(*stash), 40000 * ONE_DBC);
    assert_eq!(
        OnlineProfile::sys_info(),
        online_profile::SysInfoDetail {
            total_gpu_num: 4,
            total_calc_points: 59914,
            total_staker: 1,
            total_stake: 40000 * ONE_DBC,
            ..Default::default()
        }
    );

    // 机器恢复在线
    assert_eq!(
        OnlineProfile::live_machines(),
        LiveMachine { online_machine: vec![machine_id], ..Default::default() }
    );
}

// 空闲不足10天，块高20时报告下线，不超过7分钟报告上线 → [②] 零惩罚
#[test]
fn test_staker_report_offline() {
    new_test_with_machine_online().execute_with(|| {
        assert_idle_offline_no_slash(11 + 5 * ONE_MINUTE);
    })
}

// 空闲不足10天，下线超过7分钟不超过2天（原 4%）→ [②] 零惩罚
#[test]
fn test_staker_report_offline2() {
    new_test_with_machine_online().execute_with(|| {
        assert_idle_offline_no_slash(11 + 20 * ONE_MINUTE);
    })
}

// 空闲不足10天，下线超过2天不超过10天（原 30%）→ [②] 零惩罚
#[test]
fn test_staker_report_offline3() {
    new_test_with_machine_online().execute_with(|| {
        assert_idle_offline_no_slash(50 + 2 * ONE_DAY);
    })
}

// 空闲不足10天，下线超过10天（原 80%）→ [②] 零惩罚
#[test]
fn test_staker_report_offline4() {
    new_test_with_machine_online().execute_with(|| {
        assert_idle_offline_no_slash(50 + 10 * ONE_DAY);
    })
}
