#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::{Config, Error};
use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;

#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CustomErr {
    NotInBookList,
    TimeNotAllow,
    AlreadySubmitHash,
    AlreadySubmitRaw,
    NotSubmitHash,
    NotAllowedChangeMachineInfo,
    TelecomIsNull,
    ReportNotAllowBook,
    AlreadyBooked,
    OrderStatusNotFeat,
    NotOrderReporter,
    NotOrderCommittee,
}

impl<T: Config> From<CustomErr> for Error<T> {
    fn from(err: CustomErr) -> Self {
        match err {
            CustomErr::NotInBookList => Error::NotInBookList,
            CustomErr::TimeNotAllow => Error::TimeNotAllow,
            CustomErr::AlreadySubmitHash => Error::AlreadySubmitHash,
            CustomErr::AlreadySubmitRaw => Error::AlreadySubmitRaw,
            CustomErr::NotSubmitHash => Error::NotSubmitHash,
            CustomErr::NotAllowedChangeMachineInfo => Error::NotAllowedChangeMachineInfo,
            CustomErr::TelecomIsNull => Error::TelecomIsNull,
            CustomErr::ReportNotAllowBook => Error::ReportNotAllowBook,
            CustomErr::AlreadyBooked => Error::AlreadyBooked,
            CustomErr::OrderStatusNotFeat => Error::OrderStatusNotFeat,
            CustomErr::NotOrderReporter => Error::NotOrderReporter,
            CustomErr::NotOrderCommittee => Error::NotOrderCommittee,
        }
    }
}
