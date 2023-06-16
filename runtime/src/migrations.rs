use super::*;

pub struct RemoveCollectiveFlip;
impl frame_support::traits::OnRuntimeUpgrade for RemoveCollectiveFlip {
    fn on_runtime_upgrade() -> Weight {
        use frame_support::storage::migration;
        // Remove the storage value `RandomMaterial` from removed pallet `RandomnessCollectiveFlip`
        let _ = migration::clear_storage_prefix(
            b"RandomnessCollectiveFlip",
            b"RandomMaterial",
            b"",
            None,
            None,
        );
        <Runtime as frame_system::Config>::DbWeight::get().writes(1)
    }
}

/// Migrate from `PalletVersion` to the new `StorageVersion`
pub struct MigratePalletVersionToStorageVersion;
impl frame_support::traits::OnRuntimeUpgrade for MigratePalletVersionToStorageVersion {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        frame_support::migrations::migrate_from_pallet_version_to_storage_version::<
            AllPalletsWithSystem,
        >(&RocksDbWeight::get())
    }
}

impl frame_system::migrations::V2ToV3 for Runtime {
    type Pallet = System;
    type AccountId = AccountId;
    type Index = Index;
    type AccountData = pallet_balances::AccountData<Balance>;
}

pub struct SystemToTripleRefCount;
impl frame_support::traits::OnRuntimeUpgrade for SystemToTripleRefCount {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        frame_system::migrations::migrate_from_dual_to_triple_ref_count::<Runtime, Runtime>()
    }
}

pub struct GrandpaStoragePrefixMigration;
impl frame_support::traits::OnRuntimeUpgrade for GrandpaStoragePrefixMigration {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        use frame_support::traits::PalletInfo;
        let name = <Runtime as frame_system::Config>::PalletInfo::name::<Grandpa>()
            .expect("grandpa is part of pallets in construct_runtime, so it has a name; qed");
        pallet_grandpa::migrations::v4::migrate::<Runtime, &str>(name)
    }
}

const COUNCIL_OLD_PREFIX: &str = "Instance1Collective";
/// Migrate from `Instance1Collective` to the new pallet prefix `Council`
pub struct CouncilStoragePrefixMigration;
impl frame_support::traits::OnRuntimeUpgrade for CouncilStoragePrefixMigration {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        pallet_collective::migrations::v4::migrate::<Runtime, Council, _>(COUNCIL_OLD_PREFIX)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<(), &'static str> {
        pallet_collective::migrations::v4::pre_migrate::<Council, _>(COUNCIL_OLD_PREFIX);
        Ok(())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade() -> Result<(), &'static str> {
        pallet_collective::migrations::v4::post_migrate::<Council, _>(COUNCIL_OLD_PREFIX);
        Ok(())
    }
}

const COUNCIL_MEMBERSHIP_OLD_PREFIX: &str = "Instance1Membership";
/// Migrate from `Instance1Membership` to the new pallet prefix `TechnicalMembership`
pub struct CouncilMembershipStoragePrefixMigration;
impl frame_support::traits::OnRuntimeUpgrade for CouncilMembershipStoragePrefixMigration {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        use frame_support::traits::PalletInfo;
        let name = <Runtime as frame_system::Config>::PalletInfo::name::<TechnicalMembership>()
            .expect("CouncilMembership is part of runtime, so it has a name; qed");
        pallet_membership::migrations::v4::migrate::<Runtime, TechnicalMembership, _>(
            COUNCIL_MEMBERSHIP_OLD_PREFIX,
            name,
        )
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<(), &'static str> {
        use frame_support::traits::PalletInfo;
        let name = <Runtime as frame_system::Config>::PalletInfo::name::<TechnicalMembership>()
            .expect("CouncilMembership is part of runtime, so it has a name; qed");
        pallet_membership::migrations::v4::pre_migrate::<TechnicalMembership, _>(
            COUNCIL_MEMBERSHIP_OLD_PREFIX,
            name,
        );
        Ok(())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade() -> Result<(), &'static str> {
        use frame_support::traits::PalletInfo;
        let name = <Runtime as frame_system::Config>::PalletInfo::name::<TechnicalMembership>()
            .expect("CouncilMembership is part of runtime, so it has a name; qed");
        pallet_membership::migrations::v4::post_migrate::<TechnicalMembership, _>(
            COUNCIL_MEMBERSHIP_OLD_PREFIX,
            name,
        );
        Ok(())
    }
}

use frame_support::{traits::OnRuntimeUpgrade, weights::Weight};
pub struct CustomOnRuntimeUpgrades;
impl OnRuntimeUpgrade for CustomOnRuntimeUpgrades {
    fn on_runtime_upgrade() -> Weight {
        let mut weight = Weight::zero();

        // 1. RemoveCollectiveFlip
        frame_support::log::info!("🔍️ RemoveCollectiveFlip start");
        weight =
            weight.saturating_add(<RemoveCollectiveFlip as OnRuntimeUpgrade>::on_runtime_upgrade());
        frame_support::log::info!("🚀 RemoveCollectiveFlip end");

        // 2. MigratePalletVersionToStorageVersion
        frame_support::log::info!("🔍️ MigratePalletVersionToStorageVersion start");
        weight = weight.saturating_add(
            <MigratePalletVersionToStorageVersion as OnRuntimeUpgrade>::on_runtime_upgrade(),
        );
        frame_support::log::info!("🚀 MigratePalletVersionToStorageVersion end");

        // 3. GrandpaStoragePrefixMigration
        frame_support::log::info!("🔍️ GrandpaStoragePrefixMigration start");
        frame_support::traits::StorageVersion::new(0).put::<Grandpa>();
        weight = weight.saturating_add(
            <GrandpaStoragePrefixMigration as OnRuntimeUpgrade>::on_runtime_upgrade(),
        );
        frame_support::log::info!("🚀 GrandpaStoragePrefixMigration end");

        // 4. SystemToTripleRefCount
        frame_support::log::info!("🔍️ SystemToTripleRefCount start");
        weight = weight
            .saturating_add(<SystemToTripleRefCount as OnRuntimeUpgrade>::on_runtime_upgrade());
        frame_support::log::info!("🚀 SystemToTripleRefCount end");

        // 5. CouncilStoragePrefixMigration
        frame_support::log::info!("🔍️ CouncilStoragePrefixMigration start");
        frame_support::traits::StorageVersion::new(0).put::<Council>();
        weight = weight.saturating_add(
            <CouncilStoragePrefixMigration as OnRuntimeUpgrade>::on_runtime_upgrade(),
        );
        frame_support::log::info!("🚀 CouncilStoragePrefixMigration end");

        // 6. CouncilMembershipStoragePrefixMigration
        frame_support::log::info!("🔍️ CouncilMembershipStoragePrefixMigration start");
        frame_support::traits::StorageVersion::new(0).put::<TechnicalMembership>();
        weight +=
            <CouncilMembershipStoragePrefixMigration as OnRuntimeUpgrade>::on_runtime_upgrade();
        frame_support::log::info!("🚀 CouncilMembershipStoragePrefixMigration end");

        weight
    }
}
