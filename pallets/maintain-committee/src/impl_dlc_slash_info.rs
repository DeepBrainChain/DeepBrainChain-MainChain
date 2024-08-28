use crate::*;
use dbc_support::traits::DLCMachineSlashInfoTrait;
use sp_core::H160;
impl<T: Config> DLCMachineSlashInfoTrait for Pallet<T> {
    fn get_dlc_machine_slashed_at(machine_id: MachineId) -> u64 {
        if let Some((_, _, slash_at)) = Self::dlc_machine_2_report_info(&machine_id) {
            return slash_at
        };
        0
    }
    fn get_dlc_machine_slashed_report_id(machine_id: MachineId) -> i64 {
        if let Some((report_id, _, slash_at)) = Self::dlc_machine_2_report_info(&machine_id) {
            if slash_at > 0 {
                let report_id: u64 = report_id.into();
                return report_id as i64
            }
        }

        return -1
    }

    fn get_dlc_machine_slashed_reporter(machine_id: MachineId) -> H160 {
        if let Some((_, reporter_evm_address, slash_at)) =
            Self::dlc_machine_2_report_info(&machine_id)
        {
            if slash_at > 0 {
                return reporter_evm_address
            }
        }

        return H160::default()
    }
}

impl<T: Config> Pallet<T> {
    pub fn get_slash_report_result(
        machine_id: MachineId,
    ) -> Option<MTReportResultInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>> {
        let (report_id, _, _) = Self::dlc_machine_2_report_info(&machine_id)?;
        let report = Self::report_result(&report_id)?;
        Some(report)
    }
}
