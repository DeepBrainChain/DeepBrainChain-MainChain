#![cfg_attr(not(feature = "std"), no_std)]

pub trait DbcPrice {
    type Balance;

    fn get_dbc_amount_by_value(value: u64) -> Option<Self::Balance>;
}
