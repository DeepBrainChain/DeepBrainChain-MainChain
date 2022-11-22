pub mod committee_order;
pub mod custom_err;
pub mod live_report;
pub mod report_info;
pub mod report_result;
pub mod reporter_report;
pub mod slash_review;

pub use committee_order::*;
pub use custom_err::*;
pub use live_report::*;
pub use report_info::*;
pub use report_result::*;
pub use reporter_report::*;
pub use slash_review::*;

pub const FIVE_MINUTE: u32 = 10;
pub const TEN_MINUTE: u32 = 20;
pub const HALF_HOUR: u32 = 60;
pub const ONE_HOUR: u32 = 120;
pub const THREE_HOUR: u32 = 360;
pub const FOUR_HOUR: u32 = 480;
pub const TWO_DAY: u32 = 5760;

pub type ReportId = u64;
pub type BoxPubkey = [u8; 32];
pub type ReportHash = [u8; 16];
