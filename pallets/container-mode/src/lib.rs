// DBC container-mode pallet (MVP-1)
//
// Minimum viable chain support for container-mode GPU rentals. Single DDN authority,
// no quorum, no fault-by-ddn slashing, no challenge/rescue paths. Committee verification
// goes through DBC team multisig (CommitteeMultisigOrigin) to flip
// spec_proof_committee_verified to true.
//
// All advanced features (quorum, peer-attestation, challenge_spec_proof, RescueProposals,
// SystemFault refund, insurance pool, MachineRentHistory, image deny-list on-chain) are
// deferred to MVP-2/3.

#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

pub mod types;
pub use types::ContainerModeInfo;

/// Minimum span for a per-machine host-port pool (renter-facing SSH/HTTPS).
/// Each rental needs at most 2 ports (sshd + Jupyter); 8 leaves headroom for
/// concurrent rentals on multi-GPU hosts.
pub const MIN_PORT_RANGE: u16 = 8;

#[frame_support::pallet]
pub mod pallet {
    use super::*;
    use dbc_support::MachineId;
    use frame_support::pallet_prelude::*;
    use frame_system::pallet_prelude::*;

    const STORAGE_VERSION: StorageVersion = StorageVersion::new(0);

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config {
        type RuntimeEvent: From<Event<Self>>
            + IsType<<Self as frame_system::Config>::RuntimeEvent>;

        /// Origin for committee verification (DBC team 2-of-3 multisig in MVP-1+).
        type CommitteeMultisigOrigin: EnsureOrigin<Self::RuntimeOrigin>;

        /// Origin for emergency control: set ContainerModeEnabled, register DDN.
        /// MVP-1: same as CommitteeMultisigOrigin (DBC team multisig).
        type AdminOrigin: EnsureOrigin<Self::RuntimeOrigin>;
    }

    #[pallet::pallet]
    #[pallet::storage_version(STORAGE_VERSION)]
    // MachineId in dbc-support is `Vec<u8>` (unbounded), so MaxEncodedLen cannot be
    // derived for storage items keyed on it. The existing online-profile / rent-machine
    // pallets follow the same pattern.
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    // ---------- Storage ----------

    /// Per-machine container-mode metadata. Absence = VM mode (default).
    #[pallet::storage]
    #[pallet::getter(fn container_machines)]
    pub type ContainerModeMachines<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        ContainerModeInfo<BlockNumberFor<T>>,
        OptionQuery,
    >;

    /// The single DBC-operated DDN account allowed to call report_*_by_ddn extrinsics.
    /// MVP-1 has only one DDN; MVP-2 will replace with a BoundedVec.
    #[pallet::storage]
    #[pallet::getter(fn registered_ddn)]
    pub type RegisteredDdnAccount<T: Config> = StorageValue<_, T::AccountId, OptionQuery>;

    /// Global kill switch. Defaults to false; AdminOrigin enables before launch.
    #[pallet::storage]
    #[pallet::getter(fn container_mode_enabled)]
    pub type ContainerModeEnabled<T: Config> = StorageValue<_, bool, ValueQuery>;

    // ---------- Events ----------

    #[pallet::event]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        /// Miner requested container-mode for a machine; committee must verify next.
        ContainerModeEnableRequested {
            machine_id: MachineId,
            stash: T::AccountId,
            port_range_start: u16,
            port_range_end: u16,
        },
        /// Committee verified the machine; it can now be reported online by DDN.
        ContainerSpecVerified { machine_id: MachineId },
        /// DDN reported machine online; off-chain backend should update availability.
        /// `reporter` is the signing DDN account (forward-compatible: MVP-2 may have
        /// multiple authorised DDNs).
        MachineOnlineReported { reporter: T::AccountId, machine_id: MachineId },
        /// DDN reported machine soft-offline (no slash). Off-chain backend hides
        /// the machine from rental availability.
        MachineSoftOffline { reporter: T::AccountId, machine_id: MachineId },
        /// Admin enabled/disabled container-mode globally.
        ContainerModeEnabledChanged { enabled: bool },
        /// Admin registered (or rotated) the DDN account.
        DdnRegistered { ddn: T::AccountId },
        /// Cleanup of an entry; `removed_by` is the stash owner, or None if admin.
        ContainerModeMachineRemoved {
            machine_id: MachineId,
            removed_by: Option<T::AccountId>,
        },
    }

    // ---------- Errors ----------

    #[pallet::error]
    pub enum Error<T> {
        /// The kill switch is off; new container-mode operations are blocked.
        /// Emitted by: enable_container_mode, report_machine_online_by_ddn,
        /// report_machine_soft_offline.
        ContainerModeDisabled,
        /// Machine not registered for container mode.
        /// Emitted by: commit_container_spec_verified, report_machine_online_by_ddn,
        /// report_machine_soft_offline, remove_container_mode.
        NotContainerModeMachine,
        /// Machine already registered for container mode.
        /// Emitted by: enable_container_mode.
        AlreadyContainerModeMachine,
        /// Caller is not the owner (stash) of the machine, or the machine has been
        /// exited from online-profile (no longer in MachinesInfo).
        /// Emitted by: enable_container_mode, remove_container_mode.
        NotMachineOwner,
        /// Caller is not the registered DDN.
        /// Emitted by: report_machine_online_by_ddn, report_machine_soft_offline.
        NotRegisteredDdn,
        /// DDN account has not been registered yet.
        /// Emitted by: report_machine_online_by_ddn, report_machine_soft_offline.
        NoRegisteredDdn,
        /// Machine has not been verified by committee yet.
        /// Emitted by: report_machine_online_by_ddn.
        NotSpecVerified,
        /// Machine already verified; cannot re-verify silently.
        /// Emitted by: commit_container_spec_verified.
        AlreadyVerified,
        /// Port range invalid (end must be > start by at least MIN_PORT_RANGE).
        /// Emitted by: enable_container_mode.
        InvalidPortRange,
    }

    // ---------- Hooks (none in MVP-1) ----------

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

    // ---------- Extrinsics ----------

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// (1) Miner enables container mode for a bonded machine.
        ///
        /// Sets spec_proof_committee_verified = false; committee must later call
        /// commit_container_spec_verified to flip it.
        ///
        /// Ownership check uses online_profile::MachinesInfo (live ownership, O(1)),
        /// NOT StashMachines.total_machine which retains exited machines for 150 eras
        /// (would otherwise allow orphan-grief).
        #[pallet::call_index(0)]
        #[pallet::weight(30_000_000)]
        pub fn enable_container_mode(
            origin: OriginFor<T>,
            machine_id: MachineId,
            port_range_start: u16,
            port_range_end: u16,
        ) -> DispatchResult {
            let stash = ensure_signed(origin)?;
            Self::ensure_enabled()?;

            // u16 underflow safety: end < start would wrap in release builds.
            let delta = port_range_end
                .checked_sub(port_range_start)
                .ok_or(Error::<T>::InvalidPortRange)?;
            ensure!(delta >= MIN_PORT_RANGE, Error::<T>::InvalidPortRange);

            ensure!(
                !ContainerModeMachines::<T>::contains_key(&machine_id),
                Error::<T>::AlreadyContainerModeMachine,
            );

            // Live ownership only (see doc above).
            let machine_info = online_profile::Pallet::<T>::machines_info(&machine_id)
                .ok_or(Error::<T>::NotMachineOwner)?;
            ensure!(machine_info.machine_stash == stash, Error::<T>::NotMachineOwner);

            let now = <frame_system::Pallet<T>>::block_number();
            ContainerModeMachines::<T>::insert(
                &machine_id,
                ContainerModeInfo::<BlockNumberFor<T>> {
                    bonded_at_block: now,
                    spec_proof_committee_verified: false,
                    port_range_start,
                    port_range_end,
                },
            );

            Self::deposit_event(Event::ContainerModeEnableRequested {
                machine_id,
                stash,
                port_range_start,
                port_range_end,
            });
            Ok(())
        }

        /// (2) Committee multisig flips spec_proof_committee_verified to true.
        ///
        /// MVP-1 short-circuits the full ≥2N/3 ContainerVoteTally; team multisig acts
        /// directly. MVP-3 will replace with on-chain vote accounting.
        ///
        /// Rejects double-verification to avoid silent event spam.
        #[pallet::call_index(1)]
        #[pallet::weight(20_000_000)]
        pub fn commit_container_spec_verified(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResult {
            T::CommitteeMultisigOrigin::ensure_origin(origin)?;
            ContainerModeMachines::<T>::try_mutate(&machine_id, |maybe_info| {
                let info = maybe_info.as_mut().ok_or(Error::<T>::NotContainerModeMachine)?;
                ensure!(!info.spec_proof_committee_verified, Error::<T>::AlreadyVerified);
                info.spec_proof_committee_verified = true;
                Ok::<_, Error<T>>(())
            })?;
            Self::deposit_event(Event::ContainerSpecVerified { machine_id });
            Ok(())
        }

        /// (3) DDN reports machine online (off-chain backend updates availability).
        ///
        /// MVP-1: single DDN authority, no co-sign, no rate limit, no TTL check.
        /// Event carries `reporter` so off-chain consumers can attribute the report
        /// across DDN rotations.
        #[pallet::call_index(2)]
        #[pallet::weight(20_000_000)]
        pub fn report_machine_online_by_ddn(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResult {
            let reporter = ensure_signed(origin)?;
            Self::ensure_enabled()?;
            Self::ensure_ddn(&reporter)?;

            let info = ContainerModeMachines::<T>::get(&machine_id)
                .ok_or(Error::<T>::NotContainerModeMachine)?;
            ensure!(info.spec_proof_committee_verified, Error::<T>::NotSpecVerified);

            Self::deposit_event(Event::MachineOnlineReported { reporter, machine_id });
            Ok(())
        }

        /// (4) DDN reports machine soft-offline (no slash, just availability change).
        #[pallet::call_index(3)]
        #[pallet::weight(20_000_000)]
        pub fn report_machine_soft_offline(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResult {
            let reporter = ensure_signed(origin)?;
            Self::ensure_enabled()?;
            Self::ensure_ddn(&reporter)?;
            ensure!(
                ContainerModeMachines::<T>::contains_key(&machine_id),
                Error::<T>::NotContainerModeMachine,
            );

            Self::deposit_event(Event::MachineSoftOffline { reporter, machine_id });
            Ok(())
        }

        // ----- Stash-side cleanup -----

        /// Remove a container-mode entry (cleanup after machine exit / re-bond).
        ///
        /// Two callers are allowed:
        ///   - The current stash owner of the machine (machine still in MachinesInfo).
        ///   - `AdminOrigin` (DBC team), to clean up orphans after an online-profile exit.
        ///
        /// `try_origin` returns `Err(origin)` on failure, allowing us to fall back to
        /// `ensure_signed` for the stash path.
        #[pallet::call_index(4)]
        #[pallet::weight(15_000_000)]
        pub fn remove_container_mode(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResult {
            let removed_by: Option<T::AccountId> = match T::AdminOrigin::try_origin(origin) {
                Ok(_) => None,
                Err(o) => {
                    let who = ensure_signed(o)?;
                    let machine_info = online_profile::Pallet::<T>::machines_info(&machine_id)
                        .ok_or(Error::<T>::NotMachineOwner)?;
                    ensure!(machine_info.machine_stash == who, Error::<T>::NotMachineOwner);
                    Some(who)
                }
            };
            // `take` is atomic and returns the previous value, eliminating the
            // contains_key + remove TOCTOU pattern.
            ContainerModeMachines::<T>::take(&machine_id)
                .ok_or(Error::<T>::NotContainerModeMachine)?;
            Self::deposit_event(Event::ContainerModeMachineRemoved { machine_id, removed_by });
            Ok(())
        }

        // ----- Admin / governance extrinsics -----

        /// Admin registers (or rotates) the single DDN account. MVP-1 only.
        #[pallet::call_index(5)]
        #[pallet::weight(10_000_000)]
        pub fn set_registered_ddn(
            origin: OriginFor<T>,
            ddn: T::AccountId,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            RegisteredDdnAccount::<T>::put(&ddn);
            Self::deposit_event(Event::DdnRegistered { ddn });
            Ok(())
        }

        /// Admin global kill switch / enable.
        #[pallet::call_index(6)]
        #[pallet::weight(10_000_000)]
        pub fn set_container_mode_enabled(
            origin: OriginFor<T>,
            enabled: bool,
        ) -> DispatchResult {
            T::AdminOrigin::ensure_origin(origin)?;
            ContainerModeEnabled::<T>::put(enabled);
            Self::deposit_event(Event::ContainerModeEnabledChanged { enabled });
            Ok(())
        }
    }

    // ---------- Internal helpers (DRY) ----------

    impl<T: Config> Pallet<T> {
        fn ensure_enabled() -> DispatchResult {
            ensure!(ContainerModeEnabled::<T>::get(), Error::<T>::ContainerModeDisabled);
            Ok(())
        }

        fn ensure_ddn(who: &T::AccountId) -> DispatchResult {
            let ddn = RegisteredDdnAccount::<T>::get().ok_or(Error::<T>::NoRegisteredDdn)?;
            ensure!(who == &ddn, Error::<T>::NotRegisteredDdn);
            Ok(())
        }
    }
}

// Public query helpers for off-chain / other pallets. Placed outside the pallet module
// so they can be imported as `container_mode::Pallet::<T>::is_rentable_container(...)`.
impl<T: Config> Pallet<T> {
    /// True if `machine_id` has a container-mode entry (committee-verified or not).
    pub fn is_container_mode(machine_id: &dbc_support::MachineId) -> bool {
        ContainerModeMachines::<T>::contains_key(machine_id)
    }

    /// True iff container mode is globally enabled, `machine_id` is container-mode,
    /// AND the machine is committee-verified. This is the canonical "rentable as
    /// container" predicate for off-chain consumers.
    pub fn is_rentable_container(machine_id: &dbc_support::MachineId) -> bool {
        if !ContainerModeEnabled::<T>::get() {
            return false;
        }
        ContainerModeMachines::<T>::get(machine_id)
            .map(|info| info.spec_proof_committee_verified)
            .unwrap_or(false)
    }
}
