use frame_support::pallet_prelude::*;
use scale_info::TypeInfo;

/// Per-machine container-mode metadata.
///
/// Stored in `ContainerModeMachines` (StorageMap<MachineId, _>). Absence of an entry
/// is the canonical signal that the machine runs in VM mode.
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo, MaxEncodedLen)]
pub struct ContainerModeInfo<BlockNumber> {
    /// Block at which the miner called `enable_container_mode`.
    pub bonded_at_block: BlockNumber,
    /// True once the DBC committee multisig has verified the machine's spec proof.
    /// Only verified machines are considered rentable as container mode.
    pub spec_proof_committee_verified: bool,
    /// Inclusive lower bound of the host-port range reserved for renter ingress
    /// (SSH/HTTPS port-forwarding into Kata containers).
    pub port_range_start: u16,
    /// Exclusive upper bound — must satisfy `port_range_end - port_range_start
    /// >= MIN_PORT_RANGE` (enforced in `enable_container_mode`).
    pub port_range_end: u16,
}
