use crate::{
    mock::*,
    tests::{controller, stash},
};
use dbc_support::{
    live_machine::LiveMachine,
    verify_slash::{OPPendingSlashInfo, OPSlashReason},
    ONE_DAY, ONE_MINUTE,
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

// 空闲不足10天，块高20时报告下线，不超过7分钟报告上线
#[test]
fn test_staker_report_offline() {
    new_test_with_machine_online().execute_with(|| {
        let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
            .as_bytes()
            .to_vec();

        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));

        run_to_block(11 + 5 * ONE_MINUTE);
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));

        assert_eq!(
            OnlineProfile::pending_slash(0),
            Some(OPPendingSlashInfo {
                slash_who: *stash,
                machine_id,
                slash_time: 12 + 5 * ONE_MINUTE,
                slash_amount: 80 * ONE_DBC,
                slash_exec_time: 12 + 5 * ONE_MINUTE + 2 * ONE_DAY,
                reporter: None,
                renters: vec![],
                reward_to_committee: None,
                slash_reason: OPSlashReason::OnlineReportOffline(13),
            })
        );

        // 4000*2%=80
        assert_eq!(Balances::reserved_balance(*stash), (4000 + 80) * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), (4000 + 80) * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: (4000 + 80) * ONE_DBC,
                ..Default::default()
            }
        );

        // 两天之后，惩罚被执行
        run_to_block(11 + 5 * ONE_MINUTE + 2 * ONE_DAY);

        // 罚金4000*2%已经进入国库
        assert_eq!(Balances::reserved_balance(*stash), 4000 * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), 4000 * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: 4000 * ONE_DBC,
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
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));

        run_to_block(11 + 20 * ONE_MINUTE);
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));

        assert_eq!(
            OnlineProfile::pending_slash(0),
            Some(OPPendingSlashInfo {
                slash_who: *stash,
                machine_id,
                slash_time: 12 + 20 * ONE_MINUTE,
                slash_amount: 80 * 2 * ONE_DBC,
                slash_exec_time: 12 + 20 * ONE_MINUTE + 2 * ONE_DAY,
                reporter: None,
                renters: vec![],
                reward_to_committee: None,
                slash_reason: OPSlashReason::OnlineReportOffline(13),
            })
        );

        assert_eq!(Balances::reserved_balance(*stash), (4000 + 80 * 2) * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), (4000 + 80 * 2) * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: (4000 + 80 * 2) * ONE_DBC,
                ..Default::default()
            }
        );

        // 两天之后，惩罚被执行
        run_to_block(11 + 20 * ONE_MINUTE + 2 * ONE_DAY);

        assert_eq!(Balances::reserved_balance(*stash), 4000 * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), 4000 * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: 4000 * ONE_DBC,
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
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));

        run_to_block(50 + 2 * ONE_DAY);
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));

        assert_eq!(
            OnlineProfile::pending_slash(0),
            Some(OPPendingSlashInfo {
                slash_who: *stash,
                machine_id,
                slash_time: 51 + 2 * ONE_DAY,
                slash_amount: 80 * 15 * ONE_DBC,
                slash_exec_time: 51 + 2 * ONE_DAY + 2 * ONE_DAY,
                reporter: None,
                renters: vec![],
                reward_to_committee: None,
                slash_reason: OPSlashReason::OnlineReportOffline(13),
            })
        );

        assert_eq!(Balances::reserved_balance(*stash), (4000 + 80 * 15) * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), (4000 + 80 * 15) * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: (4000 + 80 * 15) * ONE_DBC,
                ..Default::default()
            }
        );

        // 两天之后，惩罚被执行
        run_to_block(51 + 2 * ONE_DAY + 2 * ONE_DAY);

        assert_eq!(Balances::reserved_balance(*stash), 4000 * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), 4000 * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: 4000 * ONE_DBC,
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
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));

        run_to_block(50 + 10 * ONE_DAY);

        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));
        assert_eq!(OnlineProfile::pending_exec_slash(51 + ONE_DAY * (10 + 2)), vec![0]);

        assert_eq!(
            OnlineProfile::live_machines(),
            LiveMachine { online_machine: vec![machine_id.clone()], ..Default::default() }
        );

        // 28814
        // 已经下线超过10天了，offline时间是: 13
        assert_eq!(
            OnlineProfile::pending_slash(0),
            Some(OPPendingSlashInfo {
                slash_who: *stash,
                machine_id,
                slash_time: 51 + 10 * ONE_DAY,
                slash_amount: 80 * 40 * ONE_DBC,
                slash_exec_time: 51 + 10 * ONE_DAY + 2 * ONE_DAY,
                reporter: None,
                renters: vec![],
                reward_to_committee: None,
                slash_reason: OPSlashReason::OnlineReportOffline(13),
            })
        );

        // 不存在其他的slash：
        assert_eq!(OnlineProfile::pending_slash(1), None);

        assert_eq!(Balances::reserved_balance(*stash), (4000 + 80 * 40) * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), (4000 + 80 * 40) * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: (4000 + 80 * 40) * ONE_DBC,
                ..Default::default()
            }
        );

        // 两天之后，惩罚被执行
        run_to_block(51 + 10 * ONE_DAY + 2 * ONE_DAY);

        assert_eq!(Balances::reserved_balance(*stash), 4000 * ONE_DBC);
        assert_eq!(OnlineProfile::stash_stake(*stash), 4000 * ONE_DBC);
        assert_eq!(
            OnlineProfile::sys_info(),
            online_profile::SysInfoDetail {
                total_gpu_num: 4,
                total_calc_points: 59914,
                total_staker: 1,
                total_stake: 4000 * ONE_DBC,
                ..Default::default()
            }
        );
    })
}

// // 机器上线超一年，空闲超过10天，下线，再上线不惩罚
// // 注意，不满一年，任何情况下线都是要被惩罚的
// #[test]
// fn test_staker_report_offline5() {
//     new_test_with_machine_online().execute_with(|| {
//         let machine_id = "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
//             .as_bytes()
//             .to_vec();

//         // 空闲10天
//         run_to_block(50 + 10 * ONE_DAY);

//         assert_ok!(OnlineProfile::controller_report_offline(
//             RuntimeOrigin::signed(*controller),
//             machine_id.clone()
//         ));
//         run_to_block(50 + 10 * ONE_DAY + 5);
//         assert_ok!(OnlineProfile::controller_report_online(
//             RuntimeOrigin::signed(*controller),
//             machine_id
//         ));

//         // 不存在其他的slash：
//         assert_eq!(OnlineProfile::pending_slash(0), OPPendingSlashInfo::default());
//         assert_eq!(OnlineProfile::pending_slash(1), OPPendingSlashInfo::default());

//         assert_eq!(Balances::reserved_balance(*stash), 400000 * ONE_DBC);
//         assert_eq!(OnlineProfile::stash_stake(*stash), 400000 * ONE_DBC);
//         assert_eq!(
//             OnlineProfile::sys_info(),
//             online_profile::SysInfoDetail {
//                 total_gpu_num: 4,
//                 total_calc_points: 59914,
//                 total_staker: 1,
//                 total_stake: 400000 * ONE_DBC,
//                 ..Default::default()
//             }
//         );
//     });
// }
