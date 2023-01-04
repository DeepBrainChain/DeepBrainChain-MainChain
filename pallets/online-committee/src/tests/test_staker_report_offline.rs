use crate::{
    mock::*,
    tests::{controller, stash},
};
use dbc_support::{
    live_machine::LiveMachine,
    verify_slash::{OPPendingSlashInfo, OPSlashReason},
};
use frame_support::assert_ok;

// 2). 机器在空闲状态

// 处于空闲状态不到10天，此时下线机器，没有新的在线奖励，扣除2%质押币，质押币全部进入国库。
// 处于空闲状态不到10天，此时下线机器超过7分钟，扣除4%质押币，质押币全部进入国库。
// 处于空闲状态不到10天，此时下线机器超过48h，扣除30%质押币，质押币全部进入国库。
// 处于空闲状态不到10天，此时下线机器超过240h，扣除80%质押币。质押币全部进入国库。
// 如果机器从首次上线时间起超过365天，剩下20%押金可以申请退回。 处于空闲状态超过10天，此时下线机器，
// 没有新的在线奖励，旧的奖励仍然线性释放。(机器下线后，如果机器从首次上线时间起超过365天，
// 可以申请退回押金)

// 空闲不足10天，块高20时报告下线，不超过7分钟(14个块)报告上线
#[test]
fn test_staker_report_offline() {
    new_test_with_machine_online().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        assert_ok!(OnlineProfile::controller_report_offline(
            Origin::signed(*controller),
            machine_id.clone()
        ));

        run_to_block(20);
        assert_ok!(OnlineProfile::controller_report_online(
            Origin::signed(*controller),
            machine_id.clone()
        ));

        assert_eq!(
            OnlineProfile::pending_slash(0),
            OPPendingSlashInfo {
                slash_who: *stash,
                machine_id,
                slash_time: 21,
                slash_amount: 8000 * ONE_DBC,
                slash_exec_time: 21 + 2880 * 2,
                reporter: None,
                renters: vec![],
                reward_to_committee: None,
                slash_reason: OPSlashReason::OnlineReportOffline(13),
            }
        );

        assert_eq!(Balances::reserved_balance(*stash), (400000 + 8000) * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), (400000 + 8000) * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: (400000 + 8000) * ONE_DBC,
                ..Default::default()
            }
        );

        // 两天之后，惩罚被执行
        run_to_block(21 + 2880 * 2);

        assert_eq!(Balances::reserved_balance(*stash), 400000 * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), 400000 * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: 400000 * ONE_DBC,
                ..Default::default()
            }
        );
    })
}

// 空闲不足10天，下线超过7分钟，但不超过2天，扣除4%
#[test]
fn test_staker_report_offline2() {
    new_test_with_machine_online().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        assert_ok!(OnlineProfile::controller_report_offline(
            Origin::signed(*controller),
            machine_id.clone()
        ));

        run_to_block(50);
        assert_ok!(OnlineProfile::controller_report_online(
            Origin::signed(*controller),
            machine_id.clone()
        ));

        assert_eq!(
            OnlineProfile::pending_slash(0),
            OPPendingSlashInfo {
                slash_who: *stash,
                machine_id,
                slash_time: 51,
                slash_amount: 8000 * 2 * ONE_DBC,
                slash_exec_time: 51 + 2880 * 2,
                reporter: None,
                renters: vec![],
                reward_to_committee: None,
                slash_reason: OPSlashReason::OnlineReportOffline(13),
            }
        );

        assert_eq!(Balances::reserved_balance(*stash), (400000 + 8000 * 2) * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), (400000 + 8000 * 2) * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: (400000 + 8000 * 2) * ONE_DBC,
                ..Default::default()
            }
        );

        // 两天之后，惩罚被执行
        run_to_block(51 + 2880 * 2);

        assert_eq!(Balances::reserved_balance(*stash), 400000 * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), 400000 * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: 400000 * ONE_DBC,
                ..Default::default()
            }
        );
    })
}

// 空闲不足10天，下线超过2天，不超过10天，扣除30%
#[test]
fn test_staker_report_offline3() {
    new_test_with_machine_online().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        assert_ok!(OnlineProfile::controller_report_offline(
            Origin::signed(*controller),
            machine_id.clone()
        ));

        run_to_block(50 + 2880 * 2);
        assert_ok!(OnlineProfile::controller_report_online(
            Origin::signed(*controller),
            machine_id.clone()
        ));

        assert_eq!(
            OnlineProfile::pending_slash(0),
            OPPendingSlashInfo {
                slash_who: *stash,
                machine_id,
                slash_time: 51 + 2880 * 2,
                slash_amount: 8000 * 15 * ONE_DBC,
                slash_exec_time: 51 + 2880 * 2 + 2880 * 2,
                reporter: None,
                renters: vec![],
                reward_to_committee: None,
                slash_reason: OPSlashReason::OnlineReportOffline(13),
            }
        );

        assert_eq!(Balances::reserved_balance(*stash), (400000 + 8000 * 15) * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), (400000 + 8000 * 15) * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: (400000 + 8000 * 15) * ONE_DBC,
                ..Default::default()
            }
        );

        // 两天之后，惩罚被执行
        run_to_block(51 + 2880 * 2 + 2880 * 2);

        assert_eq!(Balances::reserved_balance(*stash), 400000 * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), 400000 * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: 400000 * ONE_DBC,
                ..Default::default()
            }
        );
    })
}

// 空闲不足10天，下线超过10天，扣除80%
#[test]
fn test_staker_report_offline4() {
    new_test_with_machine_online().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        assert_ok!(OnlineProfile::controller_report_offline(
            Origin::signed(*controller),
            machine_id.clone()
        ));

        assert_eq!(
            OnlineProfile::pending_offline_slash(13 + 2880 * 10, machine_id.clone()),
            (None, vec![])
        );

        run_to_block(50 + 2880 * 10);
        assert_eq!(OnlineProfile::pending_exec_slash(13 + 2880 * (10 + 2)), vec![0]);

        assert_ok!(OnlineProfile::controller_report_online(
            Origin::signed(*controller),
            machine_id.clone()
        ));

        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { online_machine: vec![machine_id.clone()], ..Default::default() }
        );

        // 28814
        // 已经下线超过10天了，offline时间是: 13
        assert_eq!(
            OnlineProfile::pending_slash(0),
            OPPendingSlashInfo {
                slash_who: *stash,
                machine_id,
                slash_time: 13 + 2880 * 10,
                slash_amount: 8000 * 40 * ONE_DBC,
                slash_exec_time: 13 + 2880 * 10 + 2880 * 2,
                reporter: None,
                renters: vec![],
                reward_to_committee: None,
                slash_reason: OPSlashReason::OnlineReportOffline(13),
            }
        );

        // 不存在其他的slash：
        assert_eq!(OnlineProfile::pending_slash(1), OPPendingSlashInfo::default());

        assert_eq!(Balances::reserved_balance(*stash), (400000 + 8000 * 40) * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), (400000 + 8000 * 40) * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: (400000 + 8000 * 40) * ONE_DBC,
                ..Default::default()
            }
        );

        // 两天之后，惩罚被执行
        run_to_block(51 + 2880 * 10 + 2880 * 2);

        assert_eq!(Balances::reserved_balance(*stash), 400000 * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), 400000 * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: 400000 * ONE_DBC,
                ..Default::default()
            }
        );
    })
}

// 空闲超过10天，下线，再上线不扣钱
#[test]
fn test_staker_report_offline5() {
    new_test_with_machine_online().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        // 空闲10天
        run_to_block(50 + 2880 * 10);

        assert_ok!(OnlineProfile::controller_report_offline(
            Origin::signed(*controller),
            machine_id.clone()
        ));
        run_to_block(50 + 2880 * 10 + 5);
        assert_ok!(OnlineProfile::controller_report_online(
            Origin::signed(*controller),
            machine_id
        ));

        // 不存在其他的slash：
        assert_eq!(OnlineProfile::pending_slash(0), OPPendingSlashInfo::default());
        assert_eq!(OnlineProfile::pending_slash(1), OPPendingSlashInfo::default());

        assert_eq!(Balances::reserved_balance(*stash), 400000 * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), 400000 * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: 400000 * ONE_DBC,
                ..Default::default()
            }
        );
    });
}
