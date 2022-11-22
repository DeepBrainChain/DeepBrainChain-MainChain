#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::{Config, Error};
use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;
use sp_std::cmp::PartialEq;

#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CustomErr {
    OrderNotAllowBook,
    AlreadyBooked,
    NotNeedEncryptedInfo,
    NotOrderReporter,
    OrderStatusNotFeat,
    NotOrderCommittee,
    NotInBookedList,
    NotProperCommittee,
}

impl<T: Config> From<CustomErr> for Error<T> {
    fn from(err: CustomErr) -> Self {
        match err {
            CustomErr::OrderNotAllowBook => Error::OrderNotAllowBook,
            CustomErr::AlreadyBooked => Error::AlreadyBooked,
            CustomErr::NotNeedEncryptedInfo => Error::NotNeedEncryptedInfo,
            CustomErr::NotOrderReporter => Error::NotOrderReporter,
            CustomErr::OrderStatusNotFeat => Error::OrderStatusNotFeat,
            CustomErr::NotOrderCommittee => Error::NotOrderCommittee,
            CustomErr::NotInBookedList => Error::NotInBookedList,
            CustomErr::NotProperCommittee => Error::NotProperCommittee,
        }
    }
}
