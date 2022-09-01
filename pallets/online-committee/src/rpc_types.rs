use codec::{Decode, Encode};
#[cfg(feature = "std")]
use generic_func::RpcText;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};

use crate::OCCommitteeMachineList;

#[cfg(feature = "std")]
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct RpcOCCommitteeMachineList {
    pub booked_machine: Vec<RpcText>,
    pub hashed_machine: Vec<RpcText>,
    pub confirmed_machine: Vec<RpcText>,
    pub online_machine: Vec<RpcText>,
}

#[cfg(feature = "std")]
impl From<OCCommitteeMachineList> for RpcOCCommitteeMachineList {
    fn from(info: OCCommitteeMachineList) -> Self {
        Self {
            booked_machine: info.booked_machine.iter().map(|id| id.into()).collect(),
            hashed_machine: info.hashed_machine.iter().map(|id| id.into()).collect(),
            confirmed_machine: info.confirmed_machine.iter().map(|id| id.into()).collect(),
            online_machine: info.online_machine.iter().map(|id| id.into()).collect(),
        }
    }
}
