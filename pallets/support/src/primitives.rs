use sp_std::vec::Vec;

pub const ONE_MINUTE: u32 = 10;
pub const FIVE_MINUTES: u32 = 5 * ONE_MINUTE;
pub const SEVEN_MINUTES: u32 = 7 * ONE_MINUTE;

pub const HALF_HOUR: u32 = 30 * ONE_MINUTE;
pub const ONE_HOUR: u32 = 60 * ONE_MINUTE;
pub const THREE_HOURS: u32 = 3 * ONE_HOUR;
pub const FOUR_HOURS: u32 = 4 * ONE_HOUR;

pub const ONE_DAY: u32 = 24 * ONE_HOUR;
pub const TWO_DAYS: u32 = 2 * ONE_DAY;
pub const FIVE_DAYS: u32 = 5 * ONE_DAY;
pub const TEN_DAYS: u32 = 10 * ONE_DAY;

pub type SlashId = u64;
pub type MachineId = Vec<u8>;
pub type EraIndex = u32;
pub type ReportId = u64;
pub type BoxPubkey = [u8; 32];
pub type ReportHash = [u8; 16];
pub type RentOrderId = u64;

pub struct ItemList;
impl ItemList {
    pub fn add_item<T>(a_field: &mut Vec<T>, a_item: T)
    where
        T: Ord,
    {
        if let Err(index) = a_field.binary_search(&a_item) {
            a_field.insert(index, a_item);
        }
    }

    pub fn rm_item<T>(a_field: &mut Vec<T>, a_item: &T)
    where
        T: Ord,
    {
        if let Ok(index) = a_field.binary_search(a_item) {
            a_field.remove(index);
        }
    }

    pub fn expand_to_order<T>(raw_items: &mut Vec<T>, new_items: Vec<T>)
    where
        T: Ord,
    {
        for a_item in new_items {
            Self::add_item(raw_items, a_item);
        }
    }
}
