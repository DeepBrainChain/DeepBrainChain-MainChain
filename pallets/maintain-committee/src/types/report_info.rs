use dbc_support::{report::MachineFaultType, verify_slash::OPSlashReason};

pub fn into_op_err<BlockNumber>(
    fault_type: &MachineFaultType,
    report_time: BlockNumber,
) -> OPSlashReason<BlockNumber> {
    match fault_type {
        MachineFaultType::RentedInaccessible(..) => OPSlashReason::RentedInaccessible(report_time),
        MachineFaultType::RentedHardwareMalfunction(..) => {
            OPSlashReason::RentedHardwareMalfunction(report_time)
        },
        MachineFaultType::RentedHardwareCounterfeit(..) => {
            OPSlashReason::RentedHardwareCounterfeit(report_time)
        },
        MachineFaultType::OnlineRentFailed(..) => OPSlashReason::OnlineRentFailed(report_time),
    }
}
