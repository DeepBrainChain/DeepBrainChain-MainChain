pub mod test_fulfill_machine;
pub mod test_generic_destroy;
pub mod test_online_failed;
pub mod test_staker_report_offline;
pub mod test_summary;
pub mod tests;

use once_cell::sync::Lazy;
use sp_core::sr25519;
use sp_keyring::sr25519::Keyring as Sr25519Keyring;

const controller: Lazy<sp_core::sr25519::Public> = Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Eve));
const stash: Lazy<sp_core::sr25519::Public> = Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Ferdie));

const committee1: Lazy<sp_core::sr25519::Public> = Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Alice));
const committee2: Lazy<sp_core::sr25519::Public> = Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Charlie));
const committee3: Lazy<sp_core::sr25519::Public> = Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Dave));
const committee4: Lazy<sp_core::sr25519::Public> = Lazy::new(|| sr25519::Public::from(Sr25519Keyring::Eve));
