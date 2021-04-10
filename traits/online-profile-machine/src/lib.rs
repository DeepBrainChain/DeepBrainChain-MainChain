#![cfg_attr(not(feature = "std"), no_std)]

use sp_std::{collections::btree_set::BTreeSet, vec::Vec};

pub trait LCOps {
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

pub trait CommOps {
    fn vec_all_same<C: PartialEq + Copy>(arr: &[C]) -> bool;
    fn random_num(max: u32) -> u32;
    fn current_era() -> u32;
    fn block_per_era() -> u32;
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
    fn rm_bonding_id(id: Self::MachineId);
    fn add_booking_item(id: Self::MachineId, booking_item: Self::BookingItem);
    fn wallet_match_account(who: Self::AccountId, s: &Vec<u8>) -> bool;
}
