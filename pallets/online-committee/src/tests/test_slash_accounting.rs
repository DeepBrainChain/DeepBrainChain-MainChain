// spec 413 regression: Pallet::slash_and_reward must decrement the staking
// aggregates (StashStake / SysInfo.total_stake) by the amount ACTUALLY moved out
// of the stash reserve, not by the requested slash_amount.
//
// The underlying GNOps::slash_and_reward (generic-func) is best-effort: in the
// treasury path it is all-or-nothing (slashes only if reserved >= requested) and
// always returns Ok(()). The pre-413 code did `let _ = ...` then unconditionally
// subtracted the full requested amount, drifting the aggregates low whenever the
// stash reserve was short (unguarded exec_pending_slash / check_pending_slash paths).
use super::super::mock::*;
use frame_support::{assert_ok, traits::ReservableCurrency};
use online_profile::{Event as OPEvent, StashStake, SysInfo};

#[test]
fn slash_short_reserve_leaves_aggregates_and_flags_shortfall() {
    new_test_with_init_params_ext().execute_with(|| {
        System::set_block_number(1);
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);

        let recorded = 1000 * ONE_DBC; // what the aggregates believe is staked
        let reserved = 300 * ONE_DBC; // what is actually reserved (less)
        assert_ok!(<Balances as ReservableCurrency<_>>::reserve(&stash, reserved));
        StashStake::<TestRuntime>::insert(&stash, recorded);
        SysInfo::<TestRuntime>::mutate(|s| s.total_stake = recorded);

        let requested = 1000 * ONE_DBC; // > reserved => treasury path slashes NOTHING
        assert_ok!(OnlineProfile::slash_and_reward(stash, requested, vec![]));

        // Pre-413 bug: aggregates would have dropped by the full 1000. Fixed: they
        // drop by the actually-moved 0, and a shortfall event is emitted.
        assert_eq!(StashStake::<TestRuntime>::get(&stash), recorded, "StashStake must not drift");
        assert_eq!(OnlineProfile::sys_info().total_stake, recorded, "total_stake must not drift");
        assert_eq!(<Balances as ReservableCurrency<_>>::reserved_balance(&stash), reserved);

        let got_shortfall = System::events().iter().any(|e| matches!(
            &e.event,
            RuntimeEvent::OnlineProfile(OPEvent::SlashShortfall(_, req, act))
                if *req == requested && *act == 0
        ));
        assert!(got_shortfall, "expected SlashShortfall(requested=1000, actual=0)");
    })
}

#[test]
fn slash_full_reserve_decrements_fully_without_shortfall() {
    new_test_with_init_params_ext().execute_with(|| {
        System::set_block_number(1);
        let stash = sr25519::Public::from(Sr25519Keyring::Ferdie);

        let amount = 500 * ONE_DBC;
        assert_ok!(<Balances as ReservableCurrency<_>>::reserve(&stash, amount));
        StashStake::<TestRuntime>::insert(&stash, amount);
        SysInfo::<TestRuntime>::mutate(|s| s.total_stake = amount);

        assert_ok!(OnlineProfile::slash_and_reward(stash, amount, vec![]));

        // Fully reserved => full slash => aggregates go to zero, no shortfall event.
        assert_eq!(StashStake::<TestRuntime>::get(&stash), 0);
        assert_eq!(OnlineProfile::sys_info().total_stake, 0);
        let any_shortfall = System::events().iter().any(|e| matches!(
            &e.event, RuntimeEvent::OnlineProfile(OPEvent::SlashShortfall(..))));
        assert!(!any_shortfall, "no shortfall expected when fully reserved");
    })
}
