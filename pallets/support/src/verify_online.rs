use crate::{machine_type::CommitteeUploadInfo, ItemList};
use codec::{Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::{ops, vec::Vec};

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct Summary<AccountId> {
    /// Machine will be online, and those committee will get reward
    pub valid_support: Vec<AccountId>,
    /// Machine will be online, and those committee cannot get reward
    /// for they submit different message from majority committee
    pub invalid_support: Vec<AccountId>,
    /// Committees, that not submit all message
    /// such as: not submit hash, not submit raw info before deadline
    pub unruly: Vec<AccountId>,
    /// Committees, refuse machine online
    pub against: Vec<AccountId>,
    /// Raw machine info, most majority committee submit
    pub info: Option<CommitteeUploadInfo>,
}

/// What will happen after all committee submit raw machine info
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MachineConfirmStatus<AccountId> {
    /// Machine is confirmed by committee, so can be online later
    Confirmed(Summary<AccountId>),
    /// Machine is refused, will not online
    Refuse(Summary<AccountId>),
    /// No consensus, so machine will be redistributed and verified later
    NoConsensus(Summary<AccountId>),
}

impl<AccountId: Default + Clone> Default for MachineConfirmStatus<AccountId> {
    fn default() -> Self {
        Self::Confirmed(Summary { ..Default::default() })
    }
}

impl<AccountId: Clone + Ord> MachineConfirmStatus<AccountId> {
    // TODO: Refa it
    pub fn get_committee_group(self) -> (Vec<AccountId>, Vec<AccountId>, Vec<AccountId>) {
        let mut inconsistent_committee = Vec::new();
        let mut unruly_committee = Vec::new();
        let mut reward_committee = Vec::new();

        match self {
            Self::Confirmed(summary) => {
                unruly_committee = summary.unruly.clone();
                reward_committee = summary.valid_support.clone();

                for a_committee in summary.against {
                    ItemList::add_item(&mut inconsistent_committee, a_committee);
                }
                for a_committee in summary.invalid_support {
                    ItemList::add_item(&mut inconsistent_committee, a_committee);
                }
            },
            Self::NoConsensus(summary) =>
                for a_committee in summary.unruly {
                    ItemList::add_item(&mut unruly_committee, a_committee);
                },
            Self::Refuse(summary) => {
                for a_committee in summary.unruly {
                    ItemList::add_item(&mut unruly_committee, a_committee);
                }
                for a_committee in summary.invalid_support {
                    ItemList::add_item(&mut inconsistent_committee, a_committee);
                }
                for a_committee in summary.against {
                    ItemList::add_item(&mut reward_committee, a_committee);
                }
            },
        }

        (inconsistent_committee, unruly_committee, reward_committee)
    }

    pub fn into_book_result(&self) -> OCBookResultType {
        match self {
            Self::Confirmed(_) => OCBookResultType::OnlineSucceed,
            Self::Refuse(_) => OCBookResultType::OnlineRefused,
            Self::NoConsensus(_) => OCBookResultType::NoConsensus,
        }
    }

    pub fn is_refused(&self) -> bool {
        matches!(self, Self::Refuse(_))
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum OCBookResultType {
    OnlineSucceed,
    OnlineRefused,
    NoConsensus,
    // TODO: May add if is reonline
}

impl Default for OCBookResultType {
    fn default() -> Self {
        Self::OnlineRefused
    }
}
