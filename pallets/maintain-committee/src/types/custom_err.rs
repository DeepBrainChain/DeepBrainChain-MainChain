use crate::{Config, Error};
use dbc_support::report::CustomErr;

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
