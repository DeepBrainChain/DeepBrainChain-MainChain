use crate::{custom_err::VerifyErr, machine_type::CommitteeUploadInfo, ItemList, MachineId};
use codec::{Decode, Encode};
use frame_support::ensure;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::{
    traits::{CheckedAdd, Saturating, Zero},
    RuntimeDebug,
};
use sp_std::{ops, vec::Vec};

/// After order distribution 36 hours, allow committee submit raw info
pub const SUBMIT_RAW_START: u32 = 4320;
/// Summary committee's opinion after 48 hours
pub const SUBMIT_RAW_END: u32 = 5760;
/// After order distribution 36 hours, allow committee submit raw info
pub const SUBMIT_HASH_END: u32 = 4320;

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct Summary<AccountId> {
    pub valid_vote: Vec<AccountId>,
    /// Those committee cannot get reward.
    /// For they submit different message from majority committee
    pub invalid_vote: Vec<AccountId>,
    /// Committees, that not submit all message
    /// such as: not submit hash, not submit raw info before deadline
    pub unruly: Vec<AccountId>,
    /// Raw machine info, most majority committee submit
    pub info: Option<CommitteeUploadInfo>,
    pub verify_result: VerifyResult,
}

/// What will happen after all committee submit raw machine info
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum VerifyResult {
    /// Machine is confirmed by committee, so can be online later
    Confirmed,
    /// Machine is refused, will not online
    Refused,
    /// No consensus, so machine will be redistributed and verified later
    NoConsensus,
}

impl Default for VerifyResult {
    fn default() -> Self {
        Self::Confirmed
    }
}

impl<AccountId: Clone + Ord> Summary<AccountId> {
    pub fn into_book_result(&self) -> OCBookResultType {
        match self.verify_result {
            VerifyResult::Confirmed => OCBookResultType::OnlineSucceed,
            VerifyResult::Refused => OCBookResultType::OnlineRefused,
            VerifyResult::NoConsensus => OCBookResultType::NoConsensus,
        }
    }

    pub fn is_refused(&self) -> bool {
        matches!(self.verify_result, VerifyResult::Refused)
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

    pub fn submit_raw(&mut self, machine_id: MachineId) -> Result<(), VerifyErr> {
        ensure!(self.hashed_machine.binary_search(&machine_id).is_ok(), VerifyErr::NotSubmitHash);
        ensure!(
            self.confirmed_machine.binary_search(&machine_id).is_err(),
            VerifyErr::AlreadySubmitRaw
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

/// Machines' verifying committee
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct OCMachineCommitteeList<AccountId, BlockNumber> {
    /// When order distribution happened
    pub book_time: BlockNumber,
    /// Committees, get the job to verify machine info
    pub booked_committee: Vec<AccountId>,
    /// Committees, have submited machine info hash
    pub hashed_committee: Vec<AccountId>,
    /// When committee can submit raw machine info, submit machine info can
    /// immediately start after all booked_committee submit hash
    pub confirm_start_time: BlockNumber,
    /// Committees, have submit raw machine info
    pub confirmed_committee: Vec<AccountId>,
    /// Committees, get a consensus, so can get rewards after machine online
    pub onlined_committee: Vec<AccountId>,
    /// Current order status
    pub status: OCVerifyStatus,
}

impl<AccountId, BlockNumber> OCMachineCommitteeList<AccountId, BlockNumber>
where
    AccountId: Clone + Ord,
    BlockNumber: Copy + PartialOrd + ops::Add<Output = BlockNumber> + From<u32>,
{
    pub fn submit_hash_end(&self, now: BlockNumber) -> bool {
        now >= self.book_time + SUBMIT_HASH_END.into()
    }

    pub fn submit_raw_end(&self, now: BlockNumber) -> bool {
        now >= self.book_time + SUBMIT_RAW_END.into()
    }

    pub fn submit_hash(&mut self, committee: AccountId) -> Result<(), VerifyErr> {
        ensure!(self.booked_committee.binary_search(&committee).is_ok(), VerifyErr::NotInBookList);
        ensure!(
            self.hashed_committee.binary_search(&committee).is_err(),
            VerifyErr::AlreadySubmitHash
        );

        ItemList::add_item(&mut self.hashed_committee, committee);
        // 如果委员会都提交了Hash,则直接进入提交原始信息的阶段
        if self.booked_committee.len() == self.hashed_committee.len() {
            self.status = OCVerifyStatus::SubmittingRaw;
        }

        Ok(())
    }

    pub fn can_submit_raw(&self, now: BlockNumber) -> bool {
        matches!(self.status, OCVerifyStatus::SubmittingHash) &&
            now >= self.book_time + SUBMIT_RAW_START.into()
    }

    pub fn submit_raw(&mut self, time: BlockNumber, committee: AccountId) -> Result<(), VerifyErr> {
        if self.status != OCVerifyStatus::SubmittingRaw {
            ensure!(time >= self.confirm_start_time, VerifyErr::TimeNotAllow);
            ensure!(time <= self.book_time + SUBMIT_RAW_END.into(), VerifyErr::TimeNotAllow);
        }
        ensure!(self.hashed_committee.binary_search(&committee).is_ok(), VerifyErr::NotSubmitHash);

        ItemList::add_item(&mut self.confirmed_committee, committee);
        if self.confirmed_committee.len() == self.hashed_committee.len() {
            self.status = OCVerifyStatus::Summarizing;
        }
        Ok(())
    }

    // 是Summarizing的状态或 是SummitingRaw 且在有效时间内
    pub fn can_summary(&mut self, now: BlockNumber) -> bool {
        matches!(self.status, OCVerifyStatus::Summarizing) ||
            matches!(self.status, OCVerifyStatus::SubmittingRaw) &&
                now >= self.book_time + SUBMIT_RAW_END.into()
    }

    // 记录没有提交原始信息的委员会
    pub fn summary_unruly(&self) -> Vec<AccountId> {
        let mut unruly = Vec::new();
        for a_committee in self.booked_committee.clone() {
            if self.confirmed_committee.binary_search(&a_committee).is_err() {
                ItemList::add_item(&mut unruly, a_committee);
            }
        }
        unruly
    }

    pub fn after_summary(&mut self, summary_result: Summary<AccountId>) {
        match summary_result.verify_result {
            VerifyResult::Confirmed => {
                self.status = OCVerifyStatus::Finished;
                self.onlined_committee = summary_result.valid_vote;
            },
            VerifyResult::NoConsensus => {},
            VerifyResult::Refused => {
                self.status = OCVerifyStatus::Finished;
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct VerifySequence<AccountId> {
    pub who: AccountId,
    pub index: Vec<usize>,
}

/// stash account overview self-status
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct StashMachine<Balance> {
    /// All machines bonded to stash account, if machine is offline,
    /// rm from this field after 150 Eras for linear release
    pub total_machine: Vec<MachineId>,
    /// Machines, that is in passed committee verification
    pub online_machine: Vec<MachineId>,
    /// Total grades of all online machine, inflation(for multiple GPU of one stash / reward by
    /// rent) is counted
    pub total_calc_points: u64,
    /// Total online gpu num, will be added after online, reduced after offline
    pub total_gpu_num: u64,
    /// Total rented gpu
    pub total_rented_gpu: u64,
    /// All reward stash account got, locked reward included
    pub total_earned_reward: Balance,
    /// Sum of all claimed reward
    pub total_claimed_reward: Balance,
    /// Reward can be claimed now
    pub can_claim_reward: Balance,
    /// How much has been earned by rent before Galaxy is on
    pub total_rent_fee: Balance,
    /// How much has been burned after Galaxy is on
    pub total_burn_fee: Balance,
}

// In OnlineProfile pallet
impl<B: Saturating + Copy + CheckedAdd + Zero> StashMachine<B> {
    // 新加入的机器，放到total_machine中
    pub fn new_bonding(&mut self, machine_id: MachineId) {
        ItemList::add_item(&mut self.total_machine, machine_id);
    }

    pub fn update_rent_fee(&mut self, amount: B, is_burn: bool) {
        if is_burn {
            self.total_burn_fee = self.total_burn_fee.saturating_add(amount);
        } else {
            self.total_rent_fee = self.total_rent_fee.saturating_add(amount);
        }
    }

    pub fn claim_reward(&mut self) -> Result<B, ()> {
        let can_claim = self.can_claim_reward;
        self.can_claim_reward = Zero::zero();
        self.total_claimed_reward = self.total_claimed_reward.checked_add(&can_claim).ok_or(())?;
        // .ok_or(CustomErr::ClaimRewardFailed)?;
        Ok(can_claim)
    }
}

// In terminating pallet:
impl<Balance> StashMachine<Balance> {
    // 新加入的机器，放到total_machine中
    pub fn bond_machine(&mut self, machine_id: MachineId) {
        ItemList::add_item(&mut self.total_machine, machine_id);
    }

    // 拒绝machine上线
    pub fn refuse_machine(&mut self, machine_id: &MachineId) {
        ItemList::rm_item(&mut self.total_machine, machine_id);
    }

    // machine通过了委员会验证
    pub fn machine_online(&mut self, machine_id: MachineId, gpu_num: u32, calc_point: u64) {
        ItemList::add_item(&mut self.online_machine, machine_id.clone());
        self.total_gpu_num = self.total_gpu_num.saturating_add(gpu_num as u64);
        self.total_calc_points = self.total_calc_points.saturating_add(calc_point);
    }

    pub fn machine_exit(
        &mut self,
        machine_id: MachineId,
        calc_point: u64,
        gpu_count: u64,
        rented_gpu_count: u64,
    ) {
        ItemList::rm_item(&mut self.total_machine, &machine_id);
        ItemList::rm_item(&mut self.online_machine, &machine_id);
        self.total_calc_points = self.total_calc_points.saturating_sub(calc_point);
        self.total_gpu_num = self.total_gpu_num.saturating_sub(gpu_count);
        self.total_rented_gpu = self.total_rented_gpu.saturating_sub(rented_gpu_count);
    }
}
