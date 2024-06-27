use crate::{Config, Error};
use dbc_support::custom_err::VerifyErr;
use parity_scale_codec::{Decode, Encode};
use scale_info::TypeInfo;
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

/// 36 hours divide into 9 intervals for verification
pub const DISTRIBUTION: u32 = 9;

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

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug, TypeInfo)]
pub struct OCPendingSlashReviewInfo<AccountId, Balance, BlockNumber> {
    pub applicant: AccountId,
    pub staked_amount: Balance,
    pub apply_time: BlockNumber,
    pub expire_time: BlockNumber,
    pub reason: Vec<u8>,
}
