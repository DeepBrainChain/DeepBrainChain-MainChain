use sp_std::vec::Vec;

pub const FIVE_MINUTE: u32 = 10;
pub const TEN_MINUTE: u32 = 20;

pub const HALF_HOUR: u32 = 60;
pub const ONE_HOUR: u32 = 120;
pub const THREE_HOUR: u32 = 360;
pub const FOUR_HOUR: u32 = 480;

pub const TWO_DAY: u32 = 5760;

pub type SlashId = u64;
pub type MachineId = Vec<u8>;
pub type EraIndex = u32;
pub type ReportId = u64;
pub type BoxPubkey = [u8; 32];
pub type ReportHash = [u8; 16];
pub type RentOrderId = u64;
