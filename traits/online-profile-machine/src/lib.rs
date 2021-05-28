#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::vec::Vec;

// lease-committee_ops
pub trait LCOps {
    type AccountId;
    type MachineId;
    type MachineInfo;

    fn lc_booked_machine(id: Self::MachineId);
    fn lc_revert_booked_machine(id: Self::MachineId);

    fn lc_confirm_machine(who: Vec<Self::AccountId>, machine_info: Self::MachineInfo);
    fn lc_refuse_machine(who: Vec<Self::AccountId>, id: Self::MachineId);
}

pub trait OCWOps {
    type AccountId;
    type MachineId;

    fn ocw_booking_machine() -> Vec<Self::MachineId>;
    fn rm_booked_id(id: &Self::MachineId);
    fn add_ocw_confirmed_id(id: Self::MachineId, wallet: Self::AccountId);
}
