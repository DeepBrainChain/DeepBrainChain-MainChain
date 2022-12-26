use crate::{Config, Error};
use codec::{Decode, Encode};
use dbc_support::verify_online::CustomErr;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;

/// The reason why a stash account is punished
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum IRSlashReason<BlockNumber> {
    // Controller report rented machine offline
    // RentedReportOffline(BlockNumber),
    OnlineRentFailed(BlockNumber),
}

impl<T: Config> From<CustomErr> for Error<T> {
    fn from(err: CustomErr) -> Self {
        match err {
            CustomErr::NotInBookList => Error::NotInBookList,
            CustomErr::TimeNotAllow => Error::TimeNotAllow,
            CustomErr::AlreadySubmitHash => Error::AlreadySubmitHash,
            CustomErr::AlreadySubmitRaw => Error::AlreadySubmitRaw,
            CustomErr::NotSubmitHash => Error::NotSubmitHash,
            CustomErr::Overflow => Error::Overflow,
        }
    }
}
