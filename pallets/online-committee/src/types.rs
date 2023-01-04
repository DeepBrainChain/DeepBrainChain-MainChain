use crate::{Config, Error};
use codec::{Decode, Encode};
use dbc_support::custom_err::VerifyErr;
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

/// 36 hours divide into 9 intervals for verification
pub const DISTRIBUTION: u32 = 9;
/// After order distribution 36 hours, allow committee submit raw info
pub const SUBMIT_RAW_START: u32 = 4320;
/// Summary committee's opinion after 48 hours
pub const SUBMIT_RAW_END: u32 = 5760;

impl<T: Config> From<VerifyErr> for Error<T> {
    fn from(err: VerifyErr) -> Self {
        match err {
            VerifyErr::NotInBookList => Error::NotInBookList,
            VerifyErr::TimeNotAllow => Error::TimeNotAllow,
            VerifyErr::AlreadySubmitHash => Error::AlreadySubmitHash,
            VerifyErr::AlreadySubmitRaw => Error::AlreadySubmitRaw,
            VerifyErr::NotSubmitHash => Error::NotSubmitHash,
            VerifyErr::Overflow => Error::Overflow,
        }
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OCPendingSlashReviewInfo<AccountId, Balance, BlockNumber> {
    pub applicant: AccountId,
    pub staked_amount: Balance,
    pub apply_time: BlockNumber,
    pub expire_time: BlockNumber,
    pub reason: Vec<u8>,
}
