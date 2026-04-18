/// Unit tests for time-slot rental feature (分时段出租)
use crate::mock::*;
use dbc_support::ONE_DAY;
use frame_support::{assert_noop, assert_ok};
use once_cell::sync::Lazy;
use online_profile::{MachineRentalMode, TimeRange};
use sp_runtime::Perbill;

const renter_dave: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Dave));
const stash: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Ferdie));
const controller: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Eve));
const machine_id: Lazy<Vec<u8>> = Lazy::new(|| {
    "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
        .as_bytes()
        .to_vec()
});

// ═══════════════════════════════════════════════════════════════
// Rental Mode Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn default_rental_mode_is_fulltime() {
    new_test_ext_after_machine_online().execute_with(|| {
        let mode = OnlineProfile::machine_rental_mode(&*machine_id);
        assert_eq!(mode, MachineRentalMode::FullTime);
    });
}

#[test]
fn set_rental_mode_to_timeslot() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::set_machine_rental_mode(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            MachineRentalMode::TimeSlot
        ));
        assert_eq!(
            OnlineProfile::machine_rental_mode(&*machine_id),
            MachineRentalMode::TimeSlot
        );
    });
}

#[test]
fn set_rental_mode_switches_back() {
    new_test_ext_after_machine_online().execute_with(|| {
        // FullTime -> TimeSlot
        assert_ok!(OnlineProfile::set_machine_rental_mode(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            MachineRentalMode::TimeSlot
        ));
        // TimeSlot -> FullTime
        assert_ok!(OnlineProfile::set_machine_rental_mode(
            RuntimeOrigin::signed(*controller),
            machine_id.clone(),
            MachineRentalMode::FullTime
        ));
        assert_eq!(
            OnlineProfile::machine_rental_mode(&*machine_id),
            MachineRentalMode::FullTime
        );
    });
}

#[test]
fn set_rental_mode_rejects_unauthorized() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_noop!(
            OnlineProfile::set_machine_rental_mode(
                RuntimeOrigin::signed(*renter_dave),
                machine_id.clone(),
                MachineRentalMode::TimeSlot
            ),
            online_profile::Error::<TestRuntime>::NotMachineController
        );
    });
}

// ═══════════════════════════════════════════════════════════════
// Weekly Schedule Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn set_weekly_schedule_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 周一 00:00-12:00
        let ranges = vec![TimeRange { start_hour: 0, end_hour: 12 }];
        assert_ok!(OnlineProfile::set_weekly_schedule(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            1, // 周一
            ranges.clone()
        ));
        let schedule = OnlineProfile::weekly_schedule(&*machine_id);
        assert_eq!(schedule[1], ranges);
        // 其他天应为空
        assert!(schedule[0].is_empty());
        assert!(schedule[2].is_empty());
    });
}

#[test]
fn set_weekly_schedule_rejects_invalid_weekday() {
    new_test_ext_after_machine_online().execute_with(|| {
        let ranges = vec![TimeRange { start_hour: 0, end_hour: 12 }];
        assert_noop!(
            OnlineProfile::set_weekly_schedule(
                RuntimeOrigin::signed(*stash),
                machine_id.clone(),
                7, // invalid: 0-6
                ranges
            ),
            online_profile::Error::<TestRuntime>::InvalidScheduleArgs
        );
    });
}

#[test]
fn set_weekly_schedule_rejects_invalid_range() {
    new_test_ext_after_machine_online().execute_with(|| {
        // start >= end is invalid
        let bad_ranges = vec![TimeRange { start_hour: 10, end_hour: 8 }];
        assert_noop!(
            OnlineProfile::set_weekly_schedule(
                RuntimeOrigin::signed(*stash),
                machine_id.clone(),
                1,
                bad_ranges
            ),
            online_profile::Error::<TestRuntime>::InvalidScheduleArgs
        );

        // end_hour > 24 is invalid
        let bad_ranges2 = vec![TimeRange { start_hour: 10, end_hour: 25 }];
        assert_noop!(
            OnlineProfile::set_weekly_schedule(
                RuntimeOrigin::signed(*stash),
                machine_id.clone(),
                1,
                bad_ranges2
            ),
            online_profile::Error::<TestRuntime>::InvalidScheduleArgs
        );
    });
}

#[test]
fn multiple_time_ranges_in_same_day() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 一天两个时段：0-6 和 20-24
        let ranges = vec![
            TimeRange { start_hour: 0, end_hour: 6 },
            TimeRange { start_hour: 20, end_hour: 24 },
        ];
        assert_ok!(OnlineProfile::set_weekly_schedule(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            3, // 周三
            ranges.clone()
        ));
        assert_eq!(OnlineProfile::weekly_schedule(&*machine_id)[3], ranges);
    });
}

// ═══════════════════════════════════════════════════════════════
// Specific Date Schedule Tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn set_specific_date_schedule_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        let date_days = 100u32; // within test mock's time window
        let ranges = vec![TimeRange { start_hour: 14, end_hour: 18 }];
        assert_ok!(OnlineProfile::set_specific_date_schedule(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            date_days,
            ranges.clone()
        ));
        assert_eq!(
            OnlineProfile::specific_date_schedule(&*machine_id, date_days),
            ranges
        );
    });
}

#[test]
fn clear_specific_date_works() {
    new_test_ext_after_machine_online().execute_with(|| {
        let date_days = 100u32; // within test mock's time window
        let ranges = vec![TimeRange { start_hour: 14, end_hour: 18 }];
        assert_ok!(OnlineProfile::set_specific_date_schedule(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            date_days,
            ranges
        ));

        assert_ok!(OnlineProfile::clear_specific_date(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            date_days
        ));
        assert!(OnlineProfile::specific_date_schedule(&*machine_id, date_days).is_empty());
    });
}

// ═══════════════════════════════════════════════════════════════
// Time Range validation tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn time_range_is_valid() {
    let valid1 = TimeRange { start_hour: 0, end_hour: 24 };
    assert!(valid1.is_valid());
    let valid2 = TimeRange { start_hour: 9, end_hour: 17 };
    assert!(valid2.is_valid());

    let invalid1 = TimeRange { start_hour: 24, end_hour: 25 }; // start >= 24
    assert!(!invalid1.is_valid());
    let invalid2 = TimeRange { start_hour: 10, end_hour: 10 }; // start == end
    assert!(!invalid2.is_valid());
    let invalid3 = TimeRange { start_hour: 10, end_hour: 5 }; // start > end
    assert!(!invalid3.is_valid());
}

#[test]
fn time_range_covers() {
    let range = TimeRange { start_hour: 8, end_hour: 18 };
    assert!(range.covers(8, 18));   // 完全覆盖
    assert!(range.covers(10, 15));  // 子区间
    assert!(!range.covers(7, 18));  // 开始早于区间
    assert!(!range.covers(8, 19));  // 结束晚于区间
    assert!(!range.covers(20, 22)); // 完全不在
}

// ═══════════════════════════════════════════════════════════════
// Stake calculation test for TimeSlot mode
// ═══════════════════════════════════════════════════════════════

#[test]
fn stake_per_gpu_v2_time_slot_is_20000() {
    new_test_ext_after_machine_online().execute_with(|| {
        // TimeSlot 模式起步 20,000 DBC per GPU (20% of 100,000)
        let stake = OnlineProfile::stake_per_gpu_v2_time_slot().unwrap();
        assert_eq!(stake, 20_000 * ONE_DBC);
    });
}

#[test]
fn stake_per_gpu_v2_by_mode_picks_correct_value() {
    new_test_ext_after_machine_online().execute_with(|| {
        let ft_stake = OnlineProfile::stake_per_gpu_v2_by_mode(MachineRentalMode::FullTime).unwrap();
        assert_eq!(ft_stake, 10_000 * ONE_DBC);

        let ts_stake = OnlineProfile::stake_per_gpu_v2_by_mode(MachineRentalMode::TimeSlot).unwrap();
        assert_eq!(ts_stake, 20_000 * ONE_DBC);
    });
}

// ═══════════════════════════════════════════════════════════════
// Schedule validation helper tests
// ═══════════════════════════════════════════════════════════════

#[test]
fn fulltime_mode_allows_any_rental() {
    new_test_ext_after_machine_online().execute_with(|| {
        // FullTime 模式下任何时间都允许
        let start_ts: u64 = 1_700_000_000_000; // 随机时间
        let end_ts = start_ts + 3600_000; // 1 小时
        assert!(OnlineProfile::is_rental_schedule_allowed(&*machine_id, start_ts, end_ts));
    });
}

#[test]
fn timeslot_mode_rejects_rental_without_schedule() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 切到 TimeSlot 模式但没设置任何时段 → 任何租用都拒绝
        assert_ok!(OnlineProfile::set_machine_rental_mode(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            MachineRentalMode::TimeSlot
        ));
        let start_ts: u64 = 1_700_000_000_000;
        let end_ts = start_ts + 3 * 3600_000;
        assert!(!OnlineProfile::is_rental_schedule_allowed(&*machine_id, start_ts, end_ts));
    });
}

#[test]
fn timeslot_mode_rejects_too_short_rental() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::set_machine_rental_mode(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            MachineRentalMode::TimeSlot
        ));
        // 设置全天可租
        let ranges = vec![TimeRange { start_hour: 0, end_hour: 24 }];
        for wd in 0..7u8 {
            assert_ok!(OnlineProfile::set_weekly_schedule(
                RuntimeOrigin::signed(*stash),
                machine_id.clone(),
                wd,
                ranges.clone()
            ));
        }
        // 1 小时 < 2 小时最低要求
        let start_ts: u64 = 1_700_000_000_000;
        let end_ts = start_ts + 3600_000;
        assert!(!OnlineProfile::is_rental_schedule_allowed(&*machine_id, start_ts, end_ts));
    });
}

#[test]
fn specific_date_overrides_weekly_schedule() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::set_machine_rental_mode(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            MachineRentalMode::TimeSlot
        ));

        // 使用测试环境 mock 时间窗口内的日期
        // 测试 mock 起始时间 INIT_TIMESTAMP = 90_000ms，today ≈ 0
        // 选 day 100，UNIX epoch day 100 = 1970-04-11 周六 (weekday = (100+4)%7 = 6)
        let date_days = 100u32;
        let start_ts: u64 = date_days as u64 * 86400 * 1000 + 10 * 3600 * 1000; // 10:00 UTC

        // 周循环：周六只允许 00:00-06:00（不包含 10:00-12:00）
        let weekly = vec![TimeRange { start_hour: 0, end_hour: 6 }];
        assert_ok!(OnlineProfile::set_weekly_schedule(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            6, // 周六
            weekly
        ));

        // 10:00-12:00 按周循环应被拒绝
        let end_ts = start_ts + 2 * 3600_000;
        assert!(!OnlineProfile::is_rental_schedule_allowed(&*machine_id, start_ts, end_ts));

        // 为该特定日期设置覆盖：允许 10:00-14:00
        let specific = vec![TimeRange { start_hour: 10, end_hour: 14 }];
        assert_ok!(OnlineProfile::set_specific_date_schedule(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            date_days,
            specific
        ));

        // 10:00-12:00 现在应被允许
        assert!(OnlineProfile::is_rental_schedule_allowed(&*machine_id, start_ts, end_ts));
    });
}

// ═══════════════════════════════════════════════════════════════
// Additional tests from QA review
// ═══════════════════════════════════════════════════════════════

#[test]
fn empty_vec_on_set_specific_date_falls_back_to_weekly() {
    // 空 Vec 语义：删除特定日期设置，回退到每周循环
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::set_machine_rental_mode(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            MachineRentalMode::TimeSlot
        ));
        let date_days = 100u32;
        // 先设置特定日期
        assert_ok!(OnlineProfile::set_specific_date_schedule(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            date_days,
            vec![TimeRange { start_hour: 10, end_hour: 14 }]
        ));
        assert!(!OnlineProfile::specific_date_schedule(&*machine_id, date_days).is_empty());

        // 传空 Vec 应该等同于清除
        assert_ok!(OnlineProfile::set_specific_date_schedule(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            date_days,
            vec![]
        ));
        assert!(OnlineProfile::specific_date_schedule(&*machine_id, date_days).is_empty());
    });
}

#[test]
fn cannot_switch_mode_while_rented() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 先出租机器
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            10 * ONE_DAY
        ));
        run_to_block(30);
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_dave), 0));

        // 机器现在是 Rented 状态，切换模式应失败
        assert_noop!(
            OnlineProfile::set_machine_rental_mode(
                RuntimeOrigin::signed(*stash),
                machine_id.clone(),
                MachineRentalMode::TimeSlot
            ),
            online_profile::Error::<TestRuntime>::MachineStatusNotAllowed
        );
    });
}

#[test]
fn set_weekly_schedule_enforces_max_ranges() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 11 个 range 应被拒绝（上限 10）
        let too_many: Vec<TimeRange> = (0..11)
            .map(|i| TimeRange { start_hour: i * 2, end_hour: i * 2 + 1 })
            .collect();
        assert_noop!(
            OnlineProfile::set_weekly_schedule(
                RuntimeOrigin::signed(*stash),
                machine_id.clone(),
                1,
                too_many
            ),
            online_profile::Error::<TestRuntime>::InvalidScheduleArgs
        );

        // 10 个应接受
        let ok_ranges: Vec<TimeRange> = (0..10)
            .map(|i| TimeRange { start_hour: i * 2, end_hour: i * 2 + 1 })
            .collect();
        assert_ok!(OnlineProfile::set_weekly_schedule(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            1,
            ok_ranges
        ));
    });
}

#[test]
fn set_weekly_schedule_rejects_overlapping_ranges() {
    new_test_ext_after_machine_online().execute_with(|| {
        let overlapping = vec![
            TimeRange { start_hour: 0, end_hour: 12 },
            TimeRange { start_hour: 6, end_hour: 18 }, // overlaps with above
        ];
        assert_noop!(
            OnlineProfile::set_weekly_schedule(
                RuntimeOrigin::signed(*stash),
                machine_id.clone(),
                1,
                overlapping
            ),
            online_profile::Error::<TestRuntime>::InvalidScheduleArgs
        );
    });
}

#[test]
fn set_specific_date_rejects_far_future() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 超过 365 天的日期应被拒绝
        let far_future = 1000u32; // way beyond 365 days from today (which is ≈0)
        assert_noop!(
            OnlineProfile::set_specific_date_schedule(
                RuntimeOrigin::signed(*stash),
                machine_id.clone(),
                far_future,
                vec![TimeRange { start_hour: 0, end_hour: 12 }]
            ),
            online_profile::Error::<TestRuntime>::InvalidScheduleArgs
        );
    });
}

#[test]
fn set_weekly_schedule_overwrites_existing() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 先设置
        assert_ok!(OnlineProfile::set_weekly_schedule(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            1,
            vec![TimeRange { start_hour: 0, end_hour: 12 }]
        ));
        // 再设置 - 应完全覆盖
        assert_ok!(OnlineProfile::set_weekly_schedule(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            1,
            vec![TimeRange { start_hour: 14, end_hour: 20 }]
        ));
        let schedule = OnlineProfile::weekly_schedule(&*machine_id);
        assert_eq!(schedule[1], vec![TimeRange { start_hour: 14, end_hour: 20 }]);
    });
}

#[test]
fn rental_exactly_2_hours_allowed() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::set_machine_rental_mode(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            MachineRentalMode::TimeSlot
        ));
        let ranges = vec![TimeRange { start_hour: 0, end_hour: 24 }];
        for wd in 0..7u8 {
            assert_ok!(OnlineProfile::set_weekly_schedule(
                RuntimeOrigin::signed(*stash),
                machine_id.clone(),
                wd,
                ranges.clone()
            ));
        }
        let start_ts: u64 = 10 * 3600 * 1000;
        let end_ts = start_ts + 2 * 3600 * 1000; // exactly 2 hours
        assert!(OnlineProfile::is_rental_schedule_allowed(&*machine_id, start_ts, end_ts));
    });
}

#[test]
fn rental_just_under_2_hours_rejected() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::set_machine_rental_mode(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            MachineRentalMode::TimeSlot
        ));
        let ranges = vec![TimeRange { start_hour: 0, end_hour: 24 }];
        for wd in 0..7u8 {
            assert_ok!(OnlineProfile::set_weekly_schedule(
                RuntimeOrigin::signed(*stash),
                machine_id.clone(),
                wd,
                ranges.clone()
            ));
        }
        let start_ts: u64 = 10 * 3600 * 1000;
        let end_ts = start_ts + 2 * 3600 * 1000 - 1; // 1ms less than 2 hours
        assert!(!OnlineProfile::is_rental_schedule_allowed(&*machine_id, start_ts, end_ts));
    });
}

// ═══════════════════════════════════════════════════════════════
// End-to-End Integration Tests (专家审查要求)
// ═══════════════════════════════════════════════════════════════

#[test]
fn rent_machine_in_timeslot_mode_rejects_without_schedule() {
    // 机器切到 TimeSlot 但没设时段 → rent_machine 应返回 OutOfRentalSchedule
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::set_machine_rental_mode(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            MachineRentalMode::TimeSlot
        ));

        // 尝试租用 2 小时（TimeSlot 模式最小时长），无时段 → 拒绝
        // 2 小时 = 2 * 60 * 60 / 6 = 1200 blocks (DBC 主网 6 秒一个块)
        // 但 rent_machine 要求 duration 是 HALF_HOUR (300 blocks) 的整数倍
        assert_noop!(
            RentMachine::rent_machine(
                RuntimeOrigin::signed(*renter_dave),
                machine_id.clone(),
                4,
                1200u32.into() // 2 hours in blocks
            ),
            crate::Error::<TestRuntime>::OutOfRentalSchedule
        );
    });
}

#[test]
fn rent_machine_in_timeslot_mode_with_valid_schedule_succeeds() {
    // E2E: 切 TimeSlot → 设全天时段 → 租用 2 小时应成功
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::set_machine_rental_mode(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            MachineRentalMode::TimeSlot
        ));

        // 设每天全天可租
        let all_day = vec![TimeRange { start_hour: 0, end_hour: 24 }];
        for wd in 0..7u8 {
            assert_ok!(OnlineProfile::set_weekly_schedule(
                RuntimeOrigin::signed(*stash),
                machine_id.clone(),
                wd,
                all_day.clone()
            ));
        }

        // 2 小时 = 1200 blocks
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            1200u32.into()
        ));
    });
}

#[test]
fn rent_machine_in_timeslot_mode_rejects_out_of_range() {
    // E2E: 设时段 0-6，尝试在时段外租用应拒绝
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::set_machine_rental_mode(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            MachineRentalMode::TimeSlot
        ));

        // 只允许 0-6（mock 起始时间接近 0，但 rent_machine 起租时间为当前，
        // 约为 0-1 小时 UTC；rent 2 小时到 2:00 应落在 [0, 6) 内）
        let morning = vec![TimeRange { start_hour: 0, end_hour: 6 }];
        for wd in 0..7u8 {
            assert_ok!(OnlineProfile::set_weekly_schedule(
                RuntimeOrigin::signed(*stash),
                machine_id.clone(),
                wd,
                morning.clone()
            ));
        }

        // 租 10 小时（> 6 小时时段），应超出 → 拒绝
        // 10 小时 = 6000 blocks
        assert_noop!(
            RentMachine::rent_machine(
                RuntimeOrigin::signed(*renter_dave),
                machine_id.clone(),
                4,
                6000u32.into()
            ),
            crate::Error::<TestRuntime>::OutOfRentalSchedule
        );
    });
}

#[test]
fn clear_nonexistent_date_is_noop() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 清除不存在的日期应成功（幂等）
        assert_ok!(OnlineProfile::clear_specific_date(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            50u32 // never set
        ));
    });
}

#[test]
fn timeslot_rental_must_fit_in_range() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::set_machine_rental_mode(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            MachineRentalMode::TimeSlot
        ));

        // 设置周四 0-12 可租（UNIX epoch day 0 = 1970-01-01 Thursday）
        let ranges = vec![TimeRange { start_hour: 0, end_hour: 12 }];
        assert_ok!(OnlineProfile::set_weekly_schedule(
            RuntimeOrigin::signed(*stash),
            machine_id.clone(),
            4, // Thursday
            ranges
        ));

        // Day 0, 2:00-6:00 应允许
        let start_ok = 2 * 3600 * 1000u64;
        let end_ok = 6 * 3600 * 1000u64;
        assert!(OnlineProfile::is_rental_schedule_allowed(&*machine_id, start_ok, end_ok));

        // Day 0, 10:00-14:00 超出 12:00，应拒绝
        let start_bad = 10 * 3600 * 1000u64;
        let end_bad = 14 * 3600 * 1000u64;
        assert!(!OnlineProfile::is_rental_schedule_allowed(&*machine_id, start_bad, end_bad));
    });
}
