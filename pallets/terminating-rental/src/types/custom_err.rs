use crate::{Config, Error};
use dbc_support::{
    custom_err::{OnlineErr, ReportErr, VerifyErr},
    report::MachineFaultType,
    verify_slash::OPSlashReason,
};

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

impl<T: Config> From<OnlineErr> for Error<T> {
    fn from(err: OnlineErr) -> Self {
        match err {
            OnlineErr::NotAllowedChangeMachineInfo => Error::NotAllowedChangeMachineInfo,
            OnlineErr::TelecomIsNull => Error::TelecomIsNull,
        }
    }
}

impl<T: Config> From<ReportErr> for Error<T> {
    fn from(err: ReportErr) -> Self {
        match err {
            ReportErr::OrderNotAllowBook => Error::ReportNotAllowBook,
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

pub fn into_op_err<BlockNumber>(
    fault_type: &MachineFaultType,
    report_time: BlockNumber,
) -> OPSlashReason<BlockNumber> {
    match fault_type {
        MachineFaultType::RentedInaccessible(..) => OPSlashReason::RentedInaccessible(report_time),
        MachineFaultType::RentedHardwareMalfunction(..) =>
            OPSlashReason::RentedHardwareMalfunction(report_time),
        MachineFaultType::RentedHardwareCounterfeit(..) =>
            OPSlashReason::RentedHardwareCounterfeit(report_time),
        MachineFaultType::OnlineRentFailed(..) => OPSlashReason::OnlineRentFailed(report_time),
    }
}
