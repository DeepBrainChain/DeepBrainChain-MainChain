// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use super::*;
use frame_support::{log, traits::OnRuntimeUpgrade};

#[cfg(feature = "try-runtime")]
use sp_runtime::TryRuntimeError;

pub mod v1 {
    use frame_support::{pallet_prelude::*, weights::Weight};

    use super::*;

    #[derive(Decode)]
    pub struct OldCollectionDetails<AccountId, DepositBalance> {
        pub owner: AccountId,
        pub owner_deposit: DepositBalance,
        pub items: u32,
        pub item_metadatas: u32,
        pub attributes: u32,
    }

    impl<AccountId, DepositBalance> OldCollectionDetails<AccountId, DepositBalance> {
        fn migrate_to_v1(self, item_configs: u32) -> CollectionDetails<AccountId, DepositBalance> {
            CollectionDetails {
                owner: self.owner,
                owner_deposit: self.owner_deposit,
                items: self.items,
                item_metadatas: self.item_metadatas,
                item_configs,
                attributes: self.attributes,
            }
        }
    }

    pub struct MigrateToV1<T>(sp_std::marker::PhantomData<T>);
    impl<T: Config> OnRuntimeUpgrade for MigrateToV1<T> {
        fn on_runtime_upgrade() -> Weight {
            let current_version = Pallet::<T>::current_storage_version();
            let onchain_version = Pallet::<T>::on_chain_storage_version();

            log::info!(
                target: LOG_TARGET,
                "Running migration with current storage version {:?} / onchain {:?}",
                current_version,
                onchain_version
            );

            if onchain_version == 0 && current_version == 1 {
                let mut translated = 0u64;
                let mut configs_iterated = 0u64;
                Collection::<T>::translate::<
                    OldCollectionDetails<T::AccountId, DepositBalanceOf<T>>,
                    _,
                >(|key, old_value| {
                    let item_configs = ItemConfigOf::<T>::iter_prefix(&key).count() as u32;
                    configs_iterated += item_configs as u64;
                    translated.saturating_inc();
                    Some(old_value.migrate_to_v1(item_configs))
                });

                current_version.put::<Pallet<T>>();

                log::info!(
                    target: LOG_TARGET,
                    "Upgraded {} records, storage to version {:?}",
                    translated,
                    current_version
                );
                T::DbWeight::get().reads_writes(translated + configs_iterated + 1, translated + 1)
            } else {
                log::info!(
                    target: LOG_TARGET,
                    "Migration did not execute. This probably should be removed"
                );
                T::DbWeight::get().reads(1)
            }
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, TryRuntimeError> {
            let current_version = Pallet::<T>::current_storage_version();
            let onchain_version = Pallet::<T>::on_chain_storage_version();
            ensure!(onchain_version == 0 && current_version == 1, "migration from version 0 to 1.");
            let prev_count = Collection::<T>::iter().count();
            Ok((prev_count as u32).encode())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(prev_count: Vec<u8>) -> Result<(), TryRuntimeError> {
            let prev_count: u32 = Decode::decode(&mut prev_count.as_slice()).expect(
                "the state parameter should be something that was generated by pre_upgrade",
            );
            let post_count = Collection::<T>::iter().count() as u32;
            ensure!(
                prev_count == post_count,
                "the records count before and after the migration should be the same"
            );

            ensure!(Pallet::<T>::on_chain_storage_version() == 1, "wrong storage version");

            Ok(())
        }
    }
}
