use super::*;
use frame_support::{log, traits::OnRuntimeUpgrade};

pub mod v1 {
    use frame_support::{pallet_prelude::*, weights::Weight};

    use super::*;

    #[derive(Decode)]
    pub struct OldAssetDetails<Balance, AccountId, DepositBalance> {
        pub owner: AccountId,
        pub issuer: AccountId,
        pub admin: AccountId,
        pub freezer: AccountId,
        pub supply: Balance,
        pub deposit: DepositBalance,
        pub min_balance: Balance,
        pub is_sufficient: bool,
        pub accounts: u32,
        pub sufficients: u32,
        pub approvals: u32,
        pub is_frozen: bool,
    }

    impl<Balance, AccountId, DepositBalance> OldAssetDetails<Balance, AccountId, DepositBalance> {
        fn migrate_to_v1(self) -> AssetDetails<Balance, AccountId, DepositBalance> {
            let status = if self.is_frozen { AssetStatus::Frozen } else { AssetStatus::Live };

            AssetDetails {
                owner: self.owner,
                issuer: self.issuer,
                admin: self.admin,
                freezer: self.freezer,
                supply: self.supply,
                deposit: self.deposit,
                min_balance: self.min_balance,
                is_sufficient: self.is_sufficient,
                accounts: self.accounts,
                sufficients: self.sufficients,
                approvals: self.approvals,
                status,
            }
        }
    }

    #[derive(Decode)]
    pub struct OldAssetMetadata<DepositBalance> {
        deposit: DepositBalance,
        name: Vec<u8>,
        symbol: Vec<u8>,
        decimals: u8,
    }

    impl<DepositBalance>OldAssetMetadata<DepositBalance> {
        fn migrate_to_v1<BoundedString>(self, name: BoundedString, symbol: BoundedString) -> AssetMetadata<DepositBalance, BoundedString>{
            AssetMetadata {
                deposit: self.deposit,
                name: name,
                symbol: symbol,
                decimals: self.decimals,
                is_frozen: false
            }
        }
    }

    #[derive(Decode)]
    pub struct OldAssetBalance<Balance> {
        balance: Balance,
        is_frozen: bool,
        is_zombie: bool,
    }

    impl <Balance>OldAssetBalance<Balance> {
        fn migrate_to_v1<DepositBalance, Extra> (self, reason:ExistenceReason<DepositBalance> ,extra: Extra) -> AssetAccount<Balance, DepositBalance, Extra> {
            AssetAccount {
                balance: self.balance,
                is_frozen: self.is_frozen,
                // TODO:
                reason: reason,
                extra: extra,
            }
        }
    }

    pub struct MigrateToV1<T>(sp_std::marker::PhantomData<T>);
    impl<T: Config> OnRuntimeUpgrade for MigrateToV1<T> {
        fn on_runtime_upgrade() -> Weight {
            let current_version = Pallet::<T>::current_storage_version();
            let onchain_version = Pallet::<T>::on_chain_storage_version();
            if onchain_version == 0 && current_version == 1 {
                let mut translated = 0u64;
                Asset::<T>::translate::<
                    OldAssetDetails<T::Balance, T::AccountId, DepositBalanceOf<T>>,
                    _,
                >(|_key, old_value| {
                    translated.saturating_inc();
                    Some(old_value.migrate_to_v1())
                });
                Metadata::<T>::translate::<OldAssetMetadata<DepositBalanceOf<T>>,_>(|_key, old_value| {
                    // FIXME
                    let bounded_name: BoundedVec<u8, T::StringLimit> = old_value.name.clone().try_into().unwrap();
                    let bounded_symbol: BoundedVec<u8, T::StringLimit> = old_value.symbol.clone().try_into().unwrap();

                    translated.saturating_inc();
                    Some(old_value.migrate_to_v1(bounded_name, bounded_symbol))
                });
                // Account::<T>::translate::<OldAssetBalance<T::Balance>, _> (|_key1, _key2, old_value| {
                //     Some(old_value.migrate_to_v1( ))
                // });

                current_version.put::<Pallet<T>>();
                log::info!(
                    target: LOG_TARGET,
                    "Upgraded {} pools, storage to version {:?}",
                    translated,
                    current_version
                );
                T::DbWeight::get().reads_writes(translated + 1, translated + 1)
            } else {
                log::info!(
                    target: LOG_TARGET,
                    "Migration did not execute. This probably should be removed"
                );
                T::DbWeight::get().reads(1)
            }
        }

        #[cfg(feature = "try-runtime")]
        fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
            frame_support::ensure!(
                Pallet::<T>::on_chain_storage_version() == 0,
                "must upgrade linearly"
            );
            let prev_count = Asset::<T>::iter().count();
            Ok((prev_count as u32).encode())
        }

        #[cfg(feature = "try-runtime")]
        fn post_upgrade(prev_count: Vec<u8>) -> Result<(), &'static str> {
            let prev_count: u32 = Decode::decode(&mut prev_count.as_slice()).expect(
                "the state parameter should be something that was generated by pre_upgrade",
            );
            let post_count = Asset::<T>::iter().count() as u32;
            assert_eq!(
                prev_count, post_count,
                "the asset count before and after the migration should be the same"
            );

            let current_version = Pallet::<T>::current_storage_version();
            let onchain_version = Pallet::<T>::on_chain_storage_version();

            frame_support::ensure!(current_version == 1, "must_upgrade");
            assert_eq!(
                current_version, onchain_version,
                "after migration, the current_version and onchain_version should be the same"
            );

            Asset::<T>::iter().for_each(|(_id, asset)| {
                assert!(asset.status == AssetStatus::Live || asset.status == AssetStatus::Frozen, "assets should only be live or frozen. None should be in destroying status, or undefined state")
            });
            Ok(())
        }
    }
}
