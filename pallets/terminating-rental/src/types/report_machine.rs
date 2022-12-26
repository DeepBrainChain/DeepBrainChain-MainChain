use codec::{Decode, Encode};
use dbc_support::{
    report::{CustomErr as ReportErr, MachineFaultType},
    verify_slash::OPSlashReason,
    ReportHash,
};
use frame_support::ensure;
use sp_runtime::RuntimeDebug;
use sp_std::vec::Vec;

pub fn into_op_err<BlockNumber>(
    fault_type: &MachineFaultType,
    report_time: BlockNumber,
) -> OPSlashReason<BlockNumber> {
    match fault_type {
        MachineFaultType::RentedInaccessible(..) => OPSlashReason::RentedInaccessible(report_time),
        MachineFaultType::RentedHardwareMalfunction(..) =>
            OPSlashReason::RentedHardwareMalfunction(report_time),
        MachineFaultType::RentedHardwareCounterfeit(..) =>
            OPSlashReason::RentedHardwareCounterfeit(report_time),
        MachineFaultType::OnlineRentFailed(..) => OPSlashReason::OnlineRentFailed(report_time),
    }
}

/// 委员会抢单之后，对应订单的状态
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum IROrderStatus {
    /// 预订报告，状态将等待加密信息
    WaitingEncrypt,
    /// 获得加密信息之后，状态将等待加密信息
    Verifying,
    /// 等待提交原始信息
    WaitingRaw,
    /// 委员会已经完成了全部操作
    Finished,
}

impl Default for IROrderStatus {
    fn default() -> Self {
        Self::Verifying
    }
}

/// 委员会对报告的操作信息
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct IRCommitteeReportOpsDetail<BlockNumber, Balance> {
    pub booked_time: BlockNumber,
    /// reporter 提交的加密后的信息
    pub encrypted_err_info: Option<Vec<u8>>,
    pub encrypted_time: BlockNumber,
    pub confirm_hash: ReportHash,
    pub hash_time: BlockNumber,
    /// 委员会可以补充额外的信息
    pub extra_err_info: Vec<u8>,
    /// 委员会提交raw信息的时间
    pub confirm_time: BlockNumber,
    pub confirm_result: bool,
    pub staked_balance: Balance,
    pub order_status: IRReportOrderStatus,
}

impl<BlockNumber, Balance> IRCommitteeReportOpsDetail<BlockNumber, Balance> {
    pub fn add_encry_info(&mut self, info: Vec<u8>, time: BlockNumber) {
        self.encrypted_err_info = Some(info);
        self.encrypted_time = time;
        self.order_status = IRReportOrderStatus::Verifying;
    }

    pub fn book_report(
        &mut self,
        _fault_type: MachineFaultType,
        now: BlockNumber,
        order_stake: Balance,
    ) {
        // 更改committee_ps
        self.booked_time = now;
        self.order_status = IRReportOrderStatus::WaitingEncrypt;
        self.staked_balance = order_stake;

        // self.order_status = match fault_type {
        //     IRMachineFaultType::RentedInaccessible(..) => IRReportOrderStatus::Verifying,
        //     _ => {
        //         self.staked_balance = order_stake;
        //         IRReportOrderStatus::WaitingEncrypt
        //     },
        // };
    }

    pub fn can_submit_hash(&self) -> Result<(), ReportErr> {
        ensure!(self.order_status == IRReportOrderStatus::Verifying, ReportErr::OrderStatusNotFeat);
        Ok(())
    }

    pub fn add_hash(&mut self, hash: ReportHash, time: BlockNumber) {
        self.confirm_hash = hash;
        self.hash_time = time;
        self.order_status = IRReportOrderStatus::WaitingRaw;
    }
    pub fn add_raw(&mut self, time: BlockNumber, is_support: bool, extra_err_info: Vec<u8>) {
        self.confirm_time = time;
        self.extra_err_info = extra_err_info;
        self.confirm_result = is_support;
        self.order_status = IRReportOrderStatus::Finished;
    }
}

/// 委员会抢单之后，对应订单的状态
#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub enum IRReportOrderStatus {
    /// 预订报告，状态将等待加密信息
    WaitingEncrypt,
    /// 获得加密信息之后，状态将等待加密信息
    Verifying,
    /// 等待提交原始信息
    WaitingRaw,
    /// 委员会已经完成了全部操作
    Finished,
}

impl Default for IRReportOrderStatus {
    fn default() -> Self {
        Self::Verifying
    }
}
