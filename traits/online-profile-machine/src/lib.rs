#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::collections::btree_set::BTreeSet;

pub trait LCOps {
    type AccountId;
    type BlockNumber;
    type MachineId;

    // fn bonding_queue_id() -> BTreeSet<Self::MachineId>;
    // fn booking_queue_id() -> BTreeSet<Self::MachineId>;
    // fn booked_queue_id() -> BTreeSet<Self::MachineId>;
    // fn bonded_machine_id() -> BTreeSet<Self::MachineId>;
    // fn rm_booking_id(id: Self::MachineId);
    // fn add_booked_id(id: Self::MachineId);
    fn confirm_machine_grade(who: Self::AccountId, id: Self::MachineId, confirm: bool);
    // fn book_one_machine(who: &Self::AccountId, machine_id: Self::MachineId) -> bool;
}

pub trait OPOps {
    type AccountId;
    type BookingItem;
    type BondingPair;
    type ConfirmedMachine;
    type MachineId;

    fn get_bonding_pair(id: Self::MachineId) -> Self::BondingPair;
    fn add_machine_grades(id: Self::MachineId, machine_grade: Self::ConfirmedMachine);
    fn add_machine_price(id: Self::MachineId, price: u64);
    // fn rm_bonding_id(id: Self::MachineId);
    fn add_booking_item(id: Self::MachineId, booking_item: Self::BookingItem);
}

pub trait OLProof {
    type MachineId;

    fn staking_machine() -> BTreeSet<Self::MachineId>;
    fn add_verify_result(id: Self::MachineId, is_online: bool);
}
