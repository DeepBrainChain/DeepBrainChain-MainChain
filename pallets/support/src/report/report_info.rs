use crate::{BoxPubkey, MachineId, RentOrderId, ReportHash};
use codec::{Decode, Encode};
use sp_runtime::{Perbill, RuntimeDebug};
use sp_std::{cmp::PartialEq, vec};

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum MachineFaultType {
    /// 机器被租用，但无法访问的故障 (机器离线)
    RentedInaccessible(MachineId, RentOrderId),
    /// 机器被租用，但有硬件故障
    RentedHardwareMalfunction(ReportHash, BoxPubkey),
    /// 机器被租用，但硬件参数造假
    RentedHardwareCounterfeit(ReportHash, BoxPubkey),
    /// 机器是在线状态，但无法租用(创建虚拟机失败)，举报时同样需要先租下来
    OnlineRentFailed(ReportHash, BoxPubkey),
}

// 默认硬件故障
impl Default for MachineFaultType {
    fn default() -> Self {
        Self::RentedInaccessible(vec![], 0)
    }
}

impl MachineFaultType {
    pub fn get_hash(self) -> Option<ReportHash> {
        match self {
            MachineFaultType::RentedHardwareMalfunction(hash, ..) |
            MachineFaultType::RentedHardwareCounterfeit(hash, ..) |
            MachineFaultType::OnlineRentFailed(hash, ..) => Some(hash),
            MachineFaultType::RentedInaccessible(..) => None,
        }
    }

    // pub fn into_op_err<BlockNumber>(&self, report_time: BlockNumber) ->
    // OPSlashReason<BlockNumber> {     match self {
    //         Self::RentedInaccessible(..) => OPSlashReason::RentedInaccessible(report_time),
    //         Self::RentedHardwareMalfunction(..) =>
    //             OPSlashReason::RentedHardwareMalfunction(report_time),
    //         Self::RentedHardwareCounterfeit(..) =>
    //             OPSlashReason::RentedHardwareCounterfeit(report_time),
    //         Self::OnlineRentFailed(..) => OPSlashReason::OnlineRentFailed(report_time),
    //     }
    // }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum ReportStatus {
    /// 没有委员会预订过的报告, 允许报告人取消
    Reported,
    /// 前一个委员会的报告已经超过一个小时，自动改成可预订状态
    WaitingBook,
    /// 有委员会抢单，处于验证中
    Verifying,
    /// 距离第一个验证人抢单3个小时后，等待委员会上传原始信息
    SubmittingRaw,
    /// 委员会已经完成，等待第48小时, 检查报告结果
    CommitteeConfirmed,
}

impl Default for ReportStatus {
    fn default() -> Self {
        ReportStatus::Reported
    }
}

/// Reporter stake params
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct ReporterStakeParamsInfo<Balance> {
    /// First time when report
    pub stake_baseline: Balance,
    /// How much stake will be used each report & how much should stake in this
    /// module to apply for SlashReview(reporter, committee, stash stake the same)
    pub stake_per_report: Balance,
    /// 当剩余的质押数量到阈值时，需要补质押
    pub min_free_stake_percent: Perbill,
}
