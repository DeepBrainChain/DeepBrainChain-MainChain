/// [Thread B ③ · 托管] rent-machine 托管结算专项测试。
/// 覆盖：租客主动早退按已用时长比例退款、老单(无托管标记)结算 no-op(迁移边界)、
///       暂存付款领取(claim)、托管账户偿付守恒(Σ托管费 + Σ暂存 == 托管账户余额)。
use super::super::mock::*;
use crate::{
    EscrowedFee, PendingDbcPayout, RentEscrowDestroyPercent, RentInfo, TotalPendingDbcPayout,
};
use dbc_support::ONE_DAY;
use frame_support::{assert_noop, assert_ok};
use once_cell::sync::Lazy;

const renter_dave: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Dave));
const stash: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Ferdie));
const receiver_alice: Lazy<sp_core::sr25519::Public> =
    Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Alice));
const machine_id: Lazy<Vec<u8>> = Lazy::new(|| {
    "8eaf04151687736326c9fea17e25fc5287613693c912909cb226aa4794f26a48"
        .as_bytes()
        .to_vec()
});

/// 租 4 卡 10 天并确认（钱进托管）。
fn confirm_10day_rent() {
    assert_ok!(RentMachine::rent_machine(
        RuntimeOrigin::signed(*renter_dave),
        machine_id.clone(),
        4,
        10 * ONE_DAY
    ));
    run_to_block(30);
    assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_dave), 0));
}

/// 主动早退到租期中点：已用一半 → 矿工拿一半的 95%、pot 拿一半的 5%、租客退未用的一半；不罚。
#[test]
fn end_rent_midterm_refunds_unused_and_pays_used_pro_rata() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 把矿工那份租金导到独立钱包 alice，便于观测（避免落到 stash 触发补质押干扰）
        assert_ok!(OnlineProfile::set_rent_receiver(
            RuntimeOrigin::signed(*stash),
            Some(*receiver_alice),
        ));
        confirm_10day_rent();

        let info = RentMachine::rent_info(0).unwrap();
        let total_escrow = RentMachine::escrowed_fee(0);
        assert!(total_escrow > 0, "确认后托管应被注资");

        let pot = sr25519::Public::from(Sr25519Keyring::Two);
        let receiver_before = Balances::free_balance(&*receiver_alice);
        let renter_before = Balances::free_balance(&*renter_dave);
        let pot_before = Balances::free_balance(pot);

        // 跳到租期正中点（已用 50%）并主动退租
        let mid = info.rent_start + (info.rent_end - info.rent_start) / 2;
        System::set_block_number(mid);
        assert_ok!(RentMachine::end_rent(RuntimeOrigin::signed(*renter_dave), 0));

        // 托管结清并清标记、订单收尾
        assert_eq!(RentMachine::escrowed_fee(0), 0);
        assert!(!<RentInfo<TestRuntime>>::contains_key(0));

        let receiver_delta = Balances::free_balance(&*receiver_alice) - receiver_before;
        let renter_delta = Balances::free_balance(&*renter_dave) - renter_before;
        let pot_delta = Balances::free_balance(pot) - pot_before;

        // 三方都拿到钱
        assert!(receiver_delta > 0 && pot_delta > 0 && renter_delta > 0);
        // 已用部分仍是 95/5 分账
        let ratio = receiver_delta / pot_delta;
        assert!(ratio >= 18 && ratio <= 20, "已用部分应 ~19:1 (95/5)，实得 {}", ratio);
        // 退款 ≈ 未用的一半（49%~51%）
        let refund_pct = renter_delta * 100 / total_escrow;
        assert!(refund_pct >= 49 && refund_pct <= 51, "退款应 ~50%，实得 {}%", refund_pct);
        // 守恒：退款 + 已用(矿工+销毁) == 托管总额（质押 bond 全程未动）
        assert_eq!(renter_delta + receiver_delta + pot_delta, total_escrow);
    });
}

/// end_rent 权限门：非本人 / 非 Renting 状态一律拒。
#[test]
fn end_rent_rejects_non_renter_and_non_renting() {
    new_test_ext_after_machine_online().execute_with(|| {
        // 未确认(WaitingVerifying) → 拒
        assert_ok!(RentMachine::rent_machine(
            RuntimeOrigin::signed(*renter_dave),
            machine_id.clone(),
            4,
            10 * ONE_DAY
        ));
        assert_noop!(
            RentMachine::end_rent(RuntimeOrigin::signed(*renter_dave), 0),
            crate::Error::<TestRuntime>::NoOrderExist
        );
        run_to_block(30);
        assert_ok!(RentMachine::confirm_rent(RuntimeOrigin::signed(*renter_dave), 0));
        // 非租客不能退别人的租约
        assert_noop!(
            RentMachine::end_rent(RuntimeOrigin::signed(*stash), 0),
            crate::Error::<TestRuntime>::NoOrderExist
        );
    });
}

/// 迁移边界：没有 EscrowedFee 标记的订单(升级前旧单，确认时已即时付) → 结算走 no-op，不重复付款。
#[test]
fn legacy_order_without_escrow_marker_settles_as_noop() {
    new_test_ext_after_machine_online().execute_with(|| {
        assert_ok!(OnlineProfile::set_rent_receiver(
            RuntimeOrigin::signed(*stash),
            Some(*receiver_alice),
        ));
        confirm_10day_rent();

        // 模拟旧单：抹掉托管标记（旧单钱在确认时已付、无托管）
        EscrowedFee::<TestRuntime>::remove(0);
        RentEscrowDestroyPercent::<TestRuntime>::remove(0);

        let pot = sr25519::Public::from(Sr25519Keyring::Two);
        let receiver_before = Balances::free_balance(&*receiver_alice);
        let pot_before = Balances::free_balance(pot);

        settle_rent_at_end(0);

        // settle_escrow 见 total_fee==0 直接返回 → 无任何结算付款
        assert_eq!(Balances::free_balance(&*receiver_alice), receiver_before);
        assert_eq!(Balances::free_balance(pot), pot_before);
        // 但订单生命周期收尾照常（不因老单卡住清理）
        assert!(!<RentInfo<TestRuntime>>::contains_key(0));
    });
}

/// 暂存付款可领取：结算直转失败(受款方冻结/低于 ED)时钱进 PendingDbcPayout，事后 claim 从托管取回。
#[test]
fn claim_dbc_payout_pays_and_clears_then_nothing_to_claim() {
    new_test_ext_after_machine_online().execute_with(|| {
        let escrow = RentMachine::escrow_account();
        let amount = 5_000 * ONE_DBC;
        // 给托管账户注资（+ 1 DBC 缓冲，模拟托管里还有别的钱）
        assert_ok!(Balances::transfer(
            RuntimeOrigin::signed(*renter_dave),
            escrow.clone(),
            amount + ONE_DBC
        ));
        // 手工塞一笔 alice 名下的暂存付款
        PendingDbcPayout::<TestRuntime>::insert(&*receiver_alice, amount);
        TotalPendingDbcPayout::<TestRuntime>::put(amount);

        let alice_before = Balances::free_balance(&*receiver_alice);
        assert_ok!(RentMachine::claim_dbc_payout(RuntimeOrigin::signed(*receiver_alice)));

        assert_eq!(Balances::free_balance(&*receiver_alice) - alice_before, amount);
        assert_eq!(RentMachine::pending_dbc_payout(&*receiver_alice), 0);
        assert_eq!(RentMachine::total_pending_dbc_payout(), 0);
        // 再领 → 无可领
        assert_noop!(
            RentMachine::claim_dbc_payout(RuntimeOrigin::signed(*receiver_alice)),
            crate::Error::<TestRuntime>::NothingToClaim
        );
    });
}

/// 托管偿付守恒：确认后 托管账户余额 == Σ EscrowedFee + Σ PendingDbcPayout；结算后两侧同时归零。
#[test]
fn escrow_solvency_holds_through_confirm_and_settle() {
    new_test_ext_after_machine_online().execute_with(|| {
        let escrow = RentMachine::escrow_account();
        assert_eq!(Balances::free_balance(&escrow), 0, "托管账户初始为空");

        confirm_10day_rent();

        // 确认后：托管余额 == 该单托管费(+ 暂存 0)
        assert!(RentMachine::escrowed_fee(0) > 0);
        assert_eq!(
            Balances::free_balance(&escrow),
            RentMachine::escrowed_fee(0) + RentMachine::total_pending_dbc_payout()
        );

        // 到期结算后：托管清空、标记清除、暂存为 0（全额分账、无直转失败）
        settle_rent_at_end(0);
        assert_eq!(RentMachine::escrowed_fee(0), 0);
        assert_eq!(
            Balances::free_balance(&escrow),
            RentMachine::total_pending_dbc_payout()
        );
        assert_eq!(RentMachine::total_pending_dbc_payout(), 0);
    });
}
