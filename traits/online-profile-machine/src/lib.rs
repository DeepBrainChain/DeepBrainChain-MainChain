#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::vec::Vec;

// lease-committee_ops
pub trait LCOps {
    type AccountId;
    type MachineId;

    fn book_machine(id: Self::MachineId);
    fn confirm_machine_grade(who: Self::AccountId, id: Self::MachineId, confirm: bool);
    fn lc_add_booked_machine(id: Self::MachineId);
    fn lc_revert_booked_machine(id: Self::MachineId);
}

pub trait OCWOps {
    type AccountId;
    type MachineId;

    fn ocw_booking_machine() -> Vec<Self::MachineId>;
    fn rm_booked_id(id: &Self::MachineId);
    fn add_ocw_confirmed_id(id: Self::MachineId, wallet: Self::AccountId);
}
