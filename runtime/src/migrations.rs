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

    #[allow(dead_code)]
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

pub mod v2 {
    use super::*;
    use dbc_support::ItemList;

    #[allow(dead_code)]
    pub struct RentMachineMigration<T>(sp_std::marker::PhantomData<T>);
    impl<T: frame_system::Config> OnRuntimeUpgrade for RentMachineMigration<T> {
        fn on_runtime_upgrade() -> Weight {
            const FORK_BLOCK: u32 = 3683563;
            let mut total = 0;
            let now = <frame_system::Pallet<Runtime>>::block_number();
            let mut read_times = 0;
            let mut write_times = 0;

            // Remove invalid data
            rent_machine::RentEnding::<Runtime>::iter().for_each(|(rent_end, _rent_order)| {
                read_times += 1;
                write_times += 1;
                rent_machine::RentEnding::<Runtime>::remove(rent_end);
            });
            rent_machine::ConfirmingOrder::<Runtime>::iter().for_each(|(rent_end, _rent_order)| {
                read_times += 1;
                write_times += 1;
                rent_machine::ConfirmingOrder::<Runtime>::remove(rent_end);
            });

            rent_machine::RentInfo::<Runtime>::iter().for_each(|(id, info)| {
                read_times += 1;
                total += 1;

                if info.rent_end > FORK_BLOCK {
                    let new_rent_end = (info.rent_end - FORK_BLOCK) * 5;
                    // log::info!(
                    //     target: LOG_TARGET,
                    //     "RentMachineMigration migrate id {:?} rent_end {:?} to {:?}",
                    //     id, info.rent_end, new_rent_end
                    // );

                    // NOTE: If the machine rent end time is less than the current block height,
                    // it will be deleted after 100 blocks.
                    if now > new_rent_end {
                        let delay_rent_end = now + 100;
                        // log::debug!(
                        //     target: LOG_TARGET,
                        //     "RentMachineMigration migrate id {:?} new_rent_end {:?}, now {:?}, remove it on {:?}",
                        //     id, new_rent_end, now, delay_rent_end
                        // );

                        write_times += 1;
                        rent_machine::RentEnding::<Runtime>::mutate(
                            delay_rent_end,
                            |rent_ending| {
                                ItemList::add_item(rent_ending, id);
                            },
                        );
                    } else {
                        // log::debug!(
                        //     target: LOG_TARGET,
                        //     "RentMachineMigration migrate id {:?} new_rent_end {:?}, now {:?}, remove it on {:?}",
                        //     id, new_rent_end, now, new_rent_end
                        // );

                        write_times += 1;
                        rent_machine::RentEnding::<Runtime>::mutate(new_rent_end, |rent_ending| {
                            ItemList::add_item(rent_ending, id);
                        });
                    }
                } else {
                    log::warn!(
                        target: LOG_TARGET,
                        "RentMachineMigration should remove {:?}, {:?}, but keep it to check",
                        id, info
                    );
                }
            });

            log::info!(
                target: LOG_TARGET,
                "RentMachineMigration migrate {} rent order",
                total
            );

            T::DbWeight::get().reads_writes(read_times, write_times)
        }
    }
}
