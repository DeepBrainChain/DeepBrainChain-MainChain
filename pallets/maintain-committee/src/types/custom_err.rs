use crate::{Config, Error};
use dbc_support::custom_err::ReportErr;

impl<T: Config> From<ReportErr> for Error<T> {
    fn from(err: ReportErr) -> Self {
        match err {
            ReportErr::OrderNotAllowBook => Error::OrderNotAllowBook,
            ReportErr::AlreadyBooked => Error::AlreadyBooked,
            ReportErr::NotNeedEncryptedInfo => Error::NotNeedEncryptedInfo,
            ReportErr::NotOrderReporter => Error::NotOrderReporter,
            ReportErr::OrderStatusNotFeat => Error::OrderStatusNotFeat,
            ReportErr::NotOrderCommittee => Error::NotOrderCommittee,
            ReportErr::NotInBookedList => Error::NotInBookedList,
            ReportErr::NotProperCommittee => Error::NotProperCommittee,
        }
    }
}
