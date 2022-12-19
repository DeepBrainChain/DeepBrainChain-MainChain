use sp_std::vec::Vec;

pub mod live_machine;
pub mod online_verify;
pub mod stash_machine;

pub use live_machine::*;
pub use online_verify::*;
pub use stash_machine::*;

#[derive(Clone, Debug)]
pub struct VerifySequence<AccountId> {
    pub who: AccountId,
    pub index: Vec<usize>,
}
