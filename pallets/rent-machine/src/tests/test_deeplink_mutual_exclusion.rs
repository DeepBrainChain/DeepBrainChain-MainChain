/// 原生 rent-machine ↔ DeepLink(EVM RentDBC) 租用互斥约束单测
///
/// 背景：中国机器经 EVM RentDBC 合约出租 → precompile 桥 → online_profile::deeplink_set_rented，
/// 会给挖矿快照打 is_rented + 施加 +30% 被租加成。若同一机器又被原生 rent-machine 租用，则
/// is_rented / total_rented_gpu / +30% 会跨路径 double-count。两道共识守卫防止该叠加：
///   ① change_machine_status_on_rent_start：机器已被 DeepLink 租 → 拒绝原生 rent_machine
///   ② deeplink_set_rented：机器已被原生租(MachineRentedGPU>0) → 拒绝 DeepLink +30% toggle
use crate::mock::*;
use crate::WAITING_CONFIRMING_DELAY;
use dbc_support::ONE_DAY;
use frame_support::assert_ok;
use frame_support::storage::StorageMap;
use once_cell::sync::Lazy;
use online_profile::MachinesInfo;

const renter_dave: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Dave));
const controller: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Eve));
const machine_id: Lazy<Vec<u8>> = Lazy::new(|| {
    "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
        .as_bytes()
        .to_vec()
});

// ── 守卫②：机器已被原生租 → DeepLink set_rented(true) 必须 Err 且不打 is_rented ──
#[test]
fn deeplink_set_rented_rejected_when_natively_rented() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 原生租用：MachineRentedGPU>0
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            10 * ONE_DAY
        ));
        assert!(OnlineProfile::machine_rented_gpu(&*machine_id) > 0);

        // DeepLink 试图叠加 +30% → 必须被守卫②拒绝
        let r = OnlineProfile::deeplink_set_rented(machine_id.clone(), true);
        assert!(r.is_err(), "deeplink_set_rented should reject natively-rented machine");
        // 关键不变量：DeepLinkRented 仍为 false（没有 double-count）
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), false);
    });
}

// ── 守卫①：机器已被 DeepLink 租 → 原生 rent_machine 必须失败 ──
#[test]
fn native_rent_rejected_when_deeplink_rented() {
    new_test_ext_after_machine_online().execute_with(|| {
        // DeepLink 先租（无原生租用，MachineRentedGPU==0 → 守卫②放行）
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), true));
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), true);
        assert_eq!(OnlineProfile::machine_rented_gpu(&*machine_id), 0);

        // 原生 rent_machine 必须被守卫①拒绝
        let r = RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            10 * ONE_DAY,
        );
        assert!(r.is_err(), "native rent_machine should reject deeplink-rented machine");
        // 关键不变量：原生租用计数未被改动（守卫在状态变更前 return Err）
        assert_eq!(OnlineProfile::machine_rented_gpu(&*machine_id), 0);
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), true);
    });
}

// ── 幂等 + 正常 toggle：无原生租用时 DeepLink 租/退租正常，重复 set 幂等 ──
#[test]
fn deeplink_set_rented_idempotent_and_toggles_when_not_natively_rented() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 上租
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), true));
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), true);
        // 重复上租 → 幂等 Ok，仍 true（不重复加 total_rented_gpu）
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), true));
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), true);
        // 退租 → Ok，false
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), false));
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), false);
        // 重复退租 → 幂等 Ok，仍 false
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), false));
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), false);
    });
}

// ── 退租 toggle false 即使机器被原生租也放行（EVM 退租不被阻塞，守卫②只挡 is_rented=true）──
#[test]
fn deeplink_set_rented_false_allowed_even_when_natively_rented() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            10 * ONE_DAY
        ));
        // 守卫②只在 is_rented=true 时检查 MachineRentedGPU；false 路径幂等放行（DeepLinkRented 本就 false）
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), false));
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), false);
    });
}

// ════════ H-1 应急清除阀 force_set_deeplink_rented（root only）════════

// ── 卡死恢复：DeepLinkRented 卡 true → root 强制清除 → 原生租用恢复可用 ──
#[test]
fn force_set_deeplink_rented_recovers_stuck_true() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), true));
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), true);
        // 此刻原生租用被守卫①拒
        assert!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            10 * ONE_DAY
        )
        .is_err());
        // 运维强制清除（机器仍在线 → 对账成功）
        assert_ok!(OnlineProfile::force_set_deeplink_rented(
            RuntimeOrigin::root(),
            machine_id.clone(),
            false
        ));
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), false);
        // 清除后原生租用恢复
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            10 * ONE_DAY
        ));
    });
}

// ── 机器注销时 deeplink_set_rented(false) 自愈（round2：skip_snap None→Ok 清标记，EVM endRent 不再 Err 卡死）──
#[test]
fn deeplink_set_rented_false_self_heals_when_machine_deregistered() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), true));
        // 模拟机器注销：machine_info 缺失
        MachinesInfo::<TestRuntime>::remove(&*machine_id);
        // [round2] 现在普通 false 路径自愈：skip_snap(None)→只清标记→Ok（修前会 update_snap Err 卡 true）
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), false));
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), false);
    });
}

// ── 应急阀对注销机器仍能清标记（backstop）──
#[test]
fn force_set_deeplink_rented_clears_when_machine_deregistered() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), true));
        MachinesInfo::<TestRuntime>::remove(&*machine_id);
        // 直接动 storage 模拟"卡 true"（绕过自愈），验证应急阀仍能清
        assert_ok!(OnlineProfile::force_set_deeplink_rented(
            RuntimeOrigin::root(),
            machine_id.clone(),
            false
        ));
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), false);
    });
}

// ── 仅 root 可调 ──
#[test]
fn force_set_deeplink_rented_requires_root() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert!(OnlineProfile::force_set_deeplink_rented(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            false
        )
        .is_err());
    });
}

// ── 评审修：root 强制 is_rented=true 也受互斥守卫②约束（机器已被原生租→拒绝，防 double-count）──
#[test]
fn force_set_deeplink_rented_true_rejected_when_natively_rented() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            10 * ONE_DAY
        ));
        // root 强打 DeepLink 租用必须被拒（否则与原生 is_rented double-count）
        assert!(OnlineProfile::force_set_deeplink_rented(
            RuntimeOrigin::root(),
            machine_id.clone(),
            true
        )
        .is_err());
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), false);
    });
}

// ════════ #5a 会计漂移：DeepLink 租用机器 offline→online / exit 不漂移 total_rented_gpu ════════

// ── [round2 CRITICAL] DeepLink 租 → 离线 → 离线期间 EVM endRent → 上线：total_rented_gpu 不 double-减 ──
#[test]
fn deeplink_rent_offline_endrent_online_no_double_subtract() {
    new_test_ext_after_machine_online().execute_with(|| {
        let base = OnlineProfile::sys_info().total_rented_gpu;
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), true));
        assert!(OnlineProfile::sys_info().total_rented_gpu > base);
        // 离线：快照回退 total_rented_gpu→base，但 DeepLinkRented 仍 true
        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));
        assert_eq!(OnlineProfile::sys_info().total_rented_gpu, base, "offline rolled back");
        // 离线期间 EVM endRent → deeplink_set_rented(false)：必须**跳过**快照回退(机器已离线)，只清标记。
        //   修前：无离线感知 → 再回退一次 → total_rented_gpu 跌破 base(saturating 下溢) + 误扣同 stash +30%。
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), false));
        assert_eq!(
            OnlineProfile::sys_info().total_rented_gpu,
            base,
            "endRent while offline must NOT double-subtract"
        );
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), false);
        // 重新上线 → deeplink_rented 已 false → 不恢复 rented(机器已退租)，保持 base
        run_to_block(20);
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));
        assert_eq!(OnlineProfile::sys_info().total_rented_gpu, base, "stays base after endRent+online");
    });
}

// ── [round2] root force-false 离线机器：只清标记不 double-减 ──
#[test]
fn force_set_false_on_offline_machine_no_double_subtract() {
    new_test_ext_after_machine_online().execute_with(|| {
        let base = OnlineProfile::sys_info().total_rented_gpu;
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), true));
        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));
        assert_eq!(OnlineProfile::sys_info().total_rented_gpu, base, "offline rolled back");
        assert_ok!(OnlineProfile::force_set_deeplink_rented(
            RuntimeOrigin::root(),
            machine_id.clone(),
            false
        ));
        assert_eq!(OnlineProfile::sys_info().total_rented_gpu, base, "force-false offline no double-subtract");
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), false);
    });
}

// ── [round2] DeepLink 租用机器 exit 后清 DeepLinkRented 标记（防孤儿 true）──
#[test]
fn deeplink_rent_exit_clears_deeplink_rented_flag() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), true));
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), true);
        let machine_info = OnlineProfile::machines_info(&*machine_id).unwrap();
        assert_ok!(OnlineProfile::do_machine_exit(machine_id.clone(), machine_info));
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), false, "exit clears DeepLinkRented");
    });
}

// ── DeepLink 租用 → 离线回退 → 上线恢复，total_rented_gpu 全程对称无漂移 ──
#[test]
fn deeplink_rent_offline_online_no_total_rented_gpu_drift() {
    new_test_ext_after_machine_online().execute_with(|| {
        let base = OnlineProfile::sys_info().total_rented_gpu;
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), true));
        let rented = OnlineProfile::sys_info().total_rented_gpu;
        assert!(rented > base, "deeplink rent should add to total_rented_gpu");
        // 离线 → #5a 修后应回退到 base（修前 machine_status≠Rented 会跳过回退→不变）
        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));
        assert_eq!(
            OnlineProfile::sys_info().total_rented_gpu,
            base,
            "offline should roll back deeplink rented gpu"
        );
        // 重新上线 → 对称恢复到 rented
        run_to_block(20);
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));
        assert_eq!(
            OnlineProfile::sys_info().total_rented_gpu,
            rented,
            "online should restore deeplink rented gpu"
        );
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), true);
    });
}

// ── [round3] DeepLink 先离线再租用 → 上线，+30%/total_rented_gpu 只施加一次（不 double-count）──
// 复现多专家审计发现的 HIGH：旧 apply_deeplink_rented 的 is_rented=true 路径不感知离线，rent-while-offline
// 会在租用时施加一次快照、上线 controller_report_online 的 deeplink_rented 分支又施加一次 → base + 2*gpu_num，
// 触发结构上不可能的 totalRentedGpu>totalGpuNum。round3 修：true 路径离线时也跳过快照、只落标记，上线时施加恰好一次。
#[test]
fn deeplink_rent_while_offline_then_online_applies_once() {
    new_test_ext_after_machine_online().execute_with(|| {
        let base = OnlineProfile::sys_info().total_rented_gpu;
        let gpu_num = OnlineProfile::machines_info(&*machine_id).unwrap().gpu_num() as u64;
        // 先离线：此时机器未被租，machine_offline 不回退，total 保持 base
        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));
        assert_eq!(OnlineProfile::sys_info().total_rented_gpu, base);
        // 离线时把 DeepLink 租用标记打上：正常 EVM 桥路径 deeplink_set_rented 现在有在线前置校验、会拒离线机器
        //   （见 deeplink_set_rented_rejects_offline_machine）。能到达"离线 apply(true)"的只剩 root 应急阀
        //   force_set_deeplink_rented（绕过在线校验直接 apply）。它同样感知离线：round3 修后跳过快照、只落标记
        //   → total 仍 base（修前 is_rented=true 恒 apply → 此处已 = base+gpu_num，第一次误加）。
        assert_ok!(OnlineProfile::force_set_deeplink_rented(
            RuntimeOrigin::root(),
            machine_id.clone(),
            true
        ));
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), true);
        assert_eq!(
            OnlineProfile::sys_info().total_rented_gpu,
            base,
            "force-set rented while offline must DEFER the snapshot (round3 fix), not apply it"
        );
        // 重新上线 → 由 controller_report_online 的 deeplink_rented 分支恰好施加一次
        run_to_block(20);
        assert_ok!(OnlineProfile::controller_report_online(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));
        assert_eq!(
            OnlineProfile::sys_info().total_rented_gpu,
            base + gpu_num,
            "online must apply deeplink rented gpu exactly ONCE (regression: was base + 2*gpu_num)"
        );
    });
}

// ── [spec 414] precompile 在线前置校验：deeplink_set_rented(true) 对离线机器直接 Err，不落标记、不加 +30% ──
#[test]
fn deeplink_set_rented_rejects_offline_machine() {
    new_test_ext_after_machine_online().execute_with(|| {
        let base = OnlineProfile::sys_info().total_rented_gpu;
        // 机器离线
        assert_ok!(OnlineProfile::controller_report_offline(
            RuntimeOrigin::signed(*controller),
            machine_id.clone()
        ));
        // EVM 桥 setRented(true) 对离线机器 → 在线校验拒绝（Err），标记不落、快照不动
        assert!(
            OnlineProfile::deeplink_set_rented(machine_id.clone(), true).is_err(),
            "deeplink_set_rented(true) must reject an offline machine (online precondition)"
        );
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), false, "flag must NOT be set");
        assert_eq!(OnlineProfile::sys_info().total_rented_gpu, base, "no +30% applied to offline machine");
        // 反向：退租(false)不受在线校验影响（离线也能清标记）——先 force 打上再 false 清
        assert_ok!(OnlineProfile::force_set_deeplink_rented(
            RuntimeOrigin::root(),
            machine_id.clone(),
            true
        ));
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), false));
        assert_eq!(OnlineProfile::deeplink_rented(&*machine_id), false, "un-rent must always clear, even offline");
    });
}

// ── F-3：原生 reserve 后机器在 pending-confirm 期间被注销，confirm 超时仍释放 MachineRentedGPU（不泄漏）──
#[test]
fn machine_rented_gpu_not_leaked_when_deregistered_during_pending_confirm() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 原生租用 reserve：MachineRentedGPU=4，但不 confirm
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            10 * ONE_DAY
        ));
        assert_eq!(OnlineProfile::machine_rented_gpu(&*machine_id), 4);
        let b0 = System::block_number();
        // 机器在 pending-confirm 期间被注销（machine_info 缺失=F-3 卡死根因）
        MachinesInfo::<TestRuntime>::remove(&*machine_id);
        // 推进过 confirm 超时窗口 → on_finalize 触发 check_machine_starting_status → confirm_expired
        run_to_block(b0 + WAITING_CONFIRMING_DELAY + 2);
        // F-3 修后：计数器必落盘归 0（修前 machine_info 缺失致 insert 前 Err → 卡 4 → 守卫②永久拒 DeepLink）
        assert_eq!(
            OnlineProfile::machine_rented_gpu(&*machine_id),
            0,
            "MachineRentedGPU must be released even if machine deregistered during pending confirm"
        );
    });
}

// ── DeepLink 租用 → 机器退出，total_rented_gpu 回退不卡高（防 totalRentedGpu>totalGpuNum）──
#[test]
fn deeplink_rent_machine_exit_no_total_rented_gpu_drift() {
    new_test_ext_after_machine_online().execute_with(|| {
        let base = OnlineProfile::sys_info().total_rented_gpu;
        assert_ok!(OnlineProfile::deeplink_set_rented(machine_id.clone(), true));
        assert!(OnlineProfile::sys_info().total_rented_gpu > base);
        let machine_info = OnlineProfile::machines_info(&*machine_id).unwrap();
        assert_ok!(OnlineProfile::do_machine_exit(machine_id.clone(), machine_info));
        assert_eq!(
            OnlineProfile::sys_info().total_rented_gpu,
            base,
            "machine exit should roll back deeplink rented gpu (no drift)"
        );
    });
}
