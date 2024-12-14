use crate::{Runtime, Weight};
use frame_support::traits::OnRuntimeUpgrade;
use sp_core::Get;

const LOG_TARGET: &str = "runtime::migrations";

pub mod v1 {
    use super::*;
    use crate::{Assets, Balances, LockIdentifier};

    use frame_support::traits::{tokens::DepositConsequence, LockableCurrency};
    use sp_runtime::traits::Saturating;

    #[allow(dead_code)]
    pub struct AssetLockMigration<T>(sp_std::marker::PhantomData<T>);
    impl<T: frame_system::Config> OnRuntimeUpgrade for AssetLockMigration<T> {
        fn on_runtime_upgrade() -> Weight {
            const DLC: u32 = 88;
            let mut total_locks = 0;

            let _ = pallet_assets::AssetLocks::<Runtime>::clear_prefix(DLC, u32::MAX, None);

            pallet_assets::Locked::<Runtime>::iter_prefix(DLC).for_each(|(who, locked)| {
                total_locks += 1;
                if Assets::can_increase(DLC, &who, locked, true) != DepositConsequence::Success {
                    log::debug!(
                        target: LOG_TARGET,
                        "AssetLockMigration can't increase for {:?}",
                        hex::encode(&who)
                    );
                }

                pallet_assets::Locked::<Runtime>::remove(DLC, &who);

                pallet_assets::Account::<Runtime>::mutate(DLC, &who, |maybe_account| {
                    match maybe_account {
                        Some(ref mut account) => {
                            // Calculate new balance; this will not saturate since it's already checked
                            // in prep.
                            debug_assert!(
                                account.balance.checked_add(locked).is_some(),
                                "checked in prep; qed"
                            );
                            account.balance.saturating_accrue(locked);
                        },
                        maybe_account @ None => {
                            // TODO: check
                            // frame_system::Pallet::<T>::inc_consumers(who);
                            *maybe_account = Some(pallet_assets::AssetAccountOf::<Runtime, _> {
                                balance: locked,
                                reason: pallet_assets::ExistenceReason::Consumer,
                                status: pallet_assets::AccountStatus::Liquid,
                                extra: <Runtime as pallet_assets::Config>::Extra::default(),
                            });
                        },
                    }
                });

                log::debug!(
                    target: LOG_TARGET,
                    "AssetLockMigration unlocking {:?} for {:?}",
                    locked,
                    hex::encode(&who)
                );
            });

            log::info!(
                target: LOG_TARGET,
                "AssetLockMigration drained {} locks",
                total_locks
            );

            T::DbWeight::get().reads_writes(2 * total_locks, 2 * total_locks)
        }
    }

    pub struct DemocracyMigration<T>(sp_std::marker::PhantomData<T>);
    impl<T: frame_system::Config> OnRuntimeUpgrade for DemocracyMigration<T> {
        fn on_runtime_upgrade() -> Weight {
            const DEMOCRACY_ID: LockIdentifier = *b"democrac";

            let mut total = 0;
            let mut removed = 0;

            pallet_balances::Locks::<Runtime>::iter().for_each(|(who, locks)| {
                total += 1;

                locks.iter().for_each(|lock| {
                    if lock.id == DEMOCRACY_ID {
                        removed += 1;
                        log::debug!("DemocracyMigration removing lock: {:?}, {:?}", who, lock);
                        Balances::remove_lock(DEMOCRACY_ID, &who);
                    }
                });
            });

            log::info!(
                target: LOG_TARGET,
                "DemocracyMigration iter over {} locks, removed {}",
                total,
                removed
            );
            T::DbWeight::get().reads_writes(total, removed)
        }
    }
}
