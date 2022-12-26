#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;
use sp_std::cmp::PartialEq;

#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CustomErr {
    OrderNotAllowBook,
    AlreadyBooked,
    NotNeedEncryptedInfo,
    NotOrderReporter,
    OrderStatusNotFeat,
    NotOrderCommittee,
    NotInBookedList,
    NotProperCommittee,
}
