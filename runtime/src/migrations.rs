use super::*;
use frame_support::traits::GetStorageVersion;
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
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        pallet_collective::migrations::v4::pre_migrate::<Council, _>(COUNCIL_OLD_PREFIX);
        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
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
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        use frame_support::traits::PalletInfo;
        let name = <Runtime as frame_system::Config>::PalletInfo::name::<TechnicalMembership>()
            .expect("CouncilMembership is part of runtime, so it has a name; qed");
        pallet_membership::migrations::v4::pre_migrate::<TechnicalMembership, _>(
            COUNCIL_MEMBERSHIP_OLD_PREFIX,
            name,
        );
        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
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

const ELECTIONS_NEW_PREFIX: &str = "Elections";
pub struct ElectionStoragePrefixMigration;
impl frame_support::traits::OnRuntimeUpgrade for ElectionStoragePrefixMigration {
    fn on_runtime_upgrade() -> frame_support::weights::Weight {
        pallet_elections_phragmen::migrations::v4::migrate::<Runtime, &str>(ELECTIONS_NEW_PREFIX)
    }

    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        pallet_elections_phragmen::migrations::v4::pre_migration::<Runtime, &str>(
            ELECTIONS_NEW_PREFIX,
        );
        Ok(Vec::new())
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
        pallet_elections_phragmen::migrations::v4::pre_migration::<Runtime, &str>(
            ELECTIONS_NEW_PREFIX,
        );
        Ok(())
    }
}

use frame_support::{pallet_prelude::StorageVersion, traits::OnRuntimeUpgrade, weights::Weight};

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

        // 7. Elections
        frame_support::log::info!("🔍️ ElectionsStoragePrefixMigration start");
        frame_support::traits::StorageVersion::new(0).put::<Elections>();
        weight += <ElectionStoragePrefixMigration as OnRuntimeUpgrade>::on_runtime_upgrade();
        frame_support::log::info!("🚀 ElectionsStoragePrefixMigration end");

        weight
    }
}
pub struct DemocracyV1Migration;
impl OnRuntimeUpgrade for DemocracyV1Migration {
    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        StorageVersion::new(0).put::<pallet_democracy::Pallet<Runtime>>();

        let state =
            <pallet_democracy::migrations::v1::Migration<Runtime> as OnRuntimeUpgrade>::pre_upgrade(
            );

        StorageVersion::new(1).put::<pallet_democracy::Pallet<Runtime>>();
        state
    }
    fn on_runtime_upgrade() -> Weight {
        let on_chain_version = pallet_democracy::Pallet::<Runtime>::on_chain_storage_version();

        if on_chain_version != 0 {
            StorageVersion::new(0).put::<pallet_democracy::Pallet<Runtime>>();

            let weight = <pallet_democracy::migrations::v1::Migration<Runtime> as OnRuntimeUpgrade> ::on_runtime_upgrade();
            StorageVersion::new(1).put::<pallet_democracy::Pallet<Runtime>>();
            log::info!("on_runtime_upgrade ok");
            return weight
        }

        Weight::zero()
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
        <pallet_democracy::migrations::v1::Migration<Runtime> as OnRuntimeUpgrade>::post_upgrade(
            _state,
        )
    }
}

pub struct BabeV1Migration;
impl OnRuntimeUpgrade for BabeV1Migration {
    #[cfg(feature = "try-runtime")]
    fn pre_upgrade() -> Result<Vec<u8>, &'static str> {
        let timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let current_slot = pallet_babe::CurrentSlot::<Runtime>::get();
        log::info!(
            "BabeV1Migration pre_upgrade timestamp: {:?}, current slot: {:?} ",
            timestamp,
            current_slot
        );

        Ok(Vec::new())
    }
    fn on_runtime_upgrade() -> Weight {
        let weight = Weight::zero();

        let block_time = <Runtime as pallet_babe::Config>::ExpectedBlockTime::get();
        let timestamp_slot = pallet_babe::CurrentSlot::<Runtime>::get();
        log::info!(
            "BabeV1Migration block_time: {:?}, timestamp_slot: {:?}",
            block_time,
            timestamp_slot
        );

        // if block_time == 30_000 {
        {
            use sp_consensus_babe::Slot;

            let timestamp = pallet_timestamp::Pallet::<Runtime>::get() + 1;
            let slot_duration = pallet_babe::Pallet::<Runtime>::slot_duration();

            let timestamp_slot = timestamp / slot_duration;
            let timestamp_slot = Slot::from(timestamp_slot.saturated_into::<u64>());

            let before_slot = pallet_babe::CurrentSlot::<Runtime>::get();
            pallet_babe::CurrentSlot::<Runtime>::put(timestamp_slot);
            let current_slot = pallet_babe::CurrentSlot::<Runtime>::get();

            log::info!("BabeV1Migration on_runtime_upgrade ok, new timestamp_slot: {:?}, before_slot: {:?}, current_slot: {:?}", timestamp_slot, before_slot, current_slot);
            weight.saturating_add(<Runtime as frame_system::Config>::DbWeight::get().writes(1));
        }

        weight
    }

    #[cfg(feature = "try-runtime")]
    fn post_upgrade(_state: Vec<u8>) -> Result<(), &'static str> {
        let timestamp = pallet_timestamp::Pallet::<Runtime>::get();
        let new_slot = pallet_babe::CurrentSlot::<Runtime>::get();
        log::info!(
            "BabeV1Migration post_upgrade timestamp: {:?}, new slot: {:?} ",
            timestamp,
            new_slot
        );

        Ok(())
    }
}
