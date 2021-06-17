#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::vec::Vec;

// lease-committee_ops
pub trait LCOps {
    type AccountId;
    type MachineId;
    type CommitteeUploadInfo;

    fn lc_booked_machine(id: Self::MachineId);
    fn lc_revert_booked_machine(id: Self::MachineId);

    fn lc_confirm_machine(
        who: Vec<Self::AccountId>,
        machine_info: Self::CommitteeUploadInfo,
    ) -> Result<(), ()>;
    fn lc_refuse_machine(machien_id: Self::MachineId) -> Result<(), ()>;
}

pub trait RTOps {
    type AccountId;
    type MachineId;
    type MachineStatus;

    fn change_machine_status(
        machine_id: &Self::MachineId,
        new_status: Self::MachineStatus,
        renter: Self::AccountId,
    );
}
