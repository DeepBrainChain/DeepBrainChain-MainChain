use crate::{machine_type::CommitteeUploadInfo, ItemList, MachineId};
use codec::{Decode, Encode};
use frame_support::ensure;
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

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum OCVerifyStatus {
    SubmittingHash,
    SubmittingRaw,
    Summarizing,
    Finished,
}

impl Default for OCVerifyStatus {
    fn default() -> Self {
        OCVerifyStatus::SubmittingHash
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum OCMachineStatus {
    Booked,
    Hashed,
    Confirmed,
}

impl Default for OCMachineStatus {
    fn default() -> Self {
        OCMachineStatus::Booked
    }
}

/// A record of committee’s operations when verifying machine info
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct OCCommitteeOps<BlockNumber, Balance> {
    pub staked_dbc: Balance,
    /// When one committee can start the virtual machine to verify machine info
    pub verify_time: Vec<BlockNumber>,
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    /// When one committee submit raw machine info
    pub confirm_time: BlockNumber,
    pub machine_status: OCMachineStatus,
    pub machine_info: CommitteeUploadInfo,
}

impl<BlockNumber, Balance> OCCommitteeOps<BlockNumber, Balance> {
    pub fn submit_hash(&mut self, time: BlockNumber, hash: [u8; 16]) {
        self.machine_status = OCMachineStatus::Hashed;
        self.confirm_hash = hash;
        self.hash_time = time;
    }

    // 添加用户对机器的操作记录
    pub fn submit_raw(&mut self, time: BlockNumber, machine_info: CommitteeUploadInfo) {
        self.confirm_time = time;
        self.machine_status = OCMachineStatus::Confirmed;
        self.machine_info = machine_info;
        self.machine_info.rand_str = Vec::new();
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum CustomErr {
    NotInBookList,
    TimeNotAllow,
    AlreadySubmitHash,
    AlreadySubmitRaw,
    NotSubmitHash,
    Overflow,
}

/// Query distributed machines by committee address
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct OCCommitteeMachineList {
    /// machines, that distributed to committee, and should be verified
    pub booked_machine: Vec<MachineId>,
    /// machines, have submited machine info hash
    pub hashed_machine: Vec<MachineId>,
    /// machines, have submited raw machine info
    pub confirmed_machine: Vec<MachineId>,
    /// machines, online successfully
    pub online_machine: Vec<MachineId>,
}

impl OCCommitteeMachineList {
    pub fn submit_hash(&mut self, machine_id: MachineId) {
        ItemList::rm_item(&mut self.booked_machine, &machine_id);
        ItemList::add_item(&mut self.hashed_machine, machine_id);
    }

    pub fn submit_raw(&mut self, machine_id: MachineId) -> Result<(), CustomErr> {
        ensure!(self.hashed_machine.binary_search(&machine_id).is_ok(), CustomErr::NotSubmitHash);
        ensure!(
            self.confirmed_machine.binary_search(&machine_id).is_err(),
            CustomErr::AlreadySubmitRaw
        );

        ItemList::rm_item(&mut self.hashed_machine, &machine_id);
        ItemList::add_item(&mut self.confirmed_machine, machine_id);
        Ok(())
    }

    // 将要重新派单的机器从订单里清除
    pub fn revert_book(&mut self, machine_id: &MachineId) {
        ItemList::rm_item(&mut self.booked_machine, machine_id);
        ItemList::rm_item(&mut self.hashed_machine, machine_id);
        ItemList::rm_item(&mut self.confirmed_machine, machine_id);
    }

    // 机器成功上线后，从其他字段中清理掉机器记录
    // (如果未完成某一阶段的任务，机器ID将记录在那个阶段，需要进行清理)
    pub fn online_cleanup(&mut self, machine_id: &MachineId) {
        ItemList::rm_item(&mut self.booked_machine, machine_id);
        ItemList::rm_item(&mut self.hashed_machine, machine_id);
        ItemList::rm_item(&mut self.confirmed_machine, machine_id);
    }
}
