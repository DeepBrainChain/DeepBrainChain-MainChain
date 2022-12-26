use crate::{Config, Error};
use codec::{Decode, Encode};
use dbc_support::{report::MachineFaultType, verify_slash::OPSlashReason};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;

use dbc_support::report::CustomErr as ReportErr;

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
