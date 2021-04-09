#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::collections::btree_set::BTreeSet;

pub trait CommitteeMachine {
    type AccountId;
    type BlockNumber;
    type MachineId;

    fn bonding_queue_id() -> BTreeSet<Self::MachineId>;
    fn booking_queue_id() -> BTreeSet<Self::MachineId>;
    fn booked_queue_id() -> BTreeSet<Self::MachineId>;
    fn bonded_machine_id() -> BTreeSet<Self::MachineId>;
    fn rm_booking_id(id: Self::MachineId);
    fn add_booked_id(id: Self::MachineId);
    fn confirm_machine_grade(who: Self::AccountId, id: Self::MachineId, confirm: bool);

    fn book_one_machine(who: &Self::AccountId, machine_id: Self::MachineId) -> bool;
}

pub trait OnlineProfileOCW {
    type MachineId;
}
