use codec::{alloc::string::ToString, Decode, Encode};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_io::hashing::blake2_128;
use sp_runtime::{Perbill, RuntimeDebug};
use sp_std::{collections::btree_map::BTreeMap, prelude::*};

pub type MachineId = Vec<u8>;
pub type EraIndex = u32;
pub type ImageName = Vec<u8>;
pub type TelecomName = Vec<u8>;

pub const LOCK_BLOCK_EXPIRATION: u32 = 3; // in block number

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct MachineInfoDetail {
    pub committee_upload_info: CommitteeUploadInfo,
    pub staker_customize_info: StakerCustomizeInfo,
}

#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct CommitteeUploadInfo {
    pub machine_id: MachineId,
    pub gpu_type: Vec<u8>, // GPU型号
    pub gpu_num: u32,      // GPU数量
    pub cuda_core: u32,    // CUDA core数量
    pub gpu_mem: u64,      // GPU显存
    pub calc_point: u64,   // 算力值
    pub sys_disk: u64,     // 系统盘大小
    pub data_disk: u64,    // 数据盘大小
    pub cpu_type: Vec<u8>, // CPU型号
    pub cpu_core_num: u32, // CPU内核数
    pub cpu_rate: u64,     // CPU频率
    pub mem_num: u64,      // 内存数

    pub rand_str: Vec<u8>,
    pub is_support: bool, // 委员会是否支持该机器上线
}

impl CommitteeUploadInfo {
    pub fn hash(&self) -> [u8; 16] {
        let gpu_num: Vec<u8> = self.gpu_num.to_string().into();
        let cuda_core: Vec<u8> = self.cuda_core.to_string().into();
        let gpu_mem: Vec<u8> = self.gpu_mem.to_string().into();
        let calc_point: Vec<u8> = self.calc_point.to_string().into();
        let sys_disk: Vec<u8> = self.sys_disk.to_string().into();
        let data_disk: Vec<u8> = self.data_disk.to_string().into();
        let cpu_core_num: Vec<u8> = self.cpu_core_num.to_string().into();
        let cpu_rate: Vec<u8> = self.cpu_rate.to_string().into();
        let mem_num: Vec<u8> = self.mem_num.to_string().into();

        let is_support: Vec<u8> = if self.is_support { "1".into() } else { "0".into() };

        let mut raw_info = Vec::new();
        raw_info.extend(self.machine_id.clone());
        raw_info.extend(self.gpu_type.clone());
        raw_info.extend(gpu_num);
        raw_info.extend(cuda_core);
        raw_info.extend(gpu_mem);
        raw_info.extend(calc_point);
        raw_info.extend(sys_disk);
        raw_info.extend(data_disk);
        raw_info.extend(self.cpu_type.clone());
        raw_info.extend(cpu_core_num);
        raw_info.extend(cpu_rate);
        raw_info.extend(mem_num);

        raw_info.extend(self.rand_str.clone());
        raw_info.extend(is_support);

        return blake2_128(&raw_info);
    }
}

// 由机器管理者自定义的提交
#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode, Default)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct StakerCustomizeInfo {
    /// 上行带宽
    pub upload_net: u64,
    /// 下行带宽
    pub download_net: u64,
    /// 经度(+东经; -西经)
    pub longitude: i64,
    /// 纬度(+北纬； -南纬)
    pub latitude: i64,
    /// 网络运营商
    pub telecom_operators: Vec<Vec<u8>>,
    /// 镜像名称
    pub images: Vec<ImageName>,
}

/// 记录每个Era的机器的总分
/// NOTE: 这个账户应该是stash账户，而不是controller账户
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct EraStashPoints<AccountId: Ord> {
    /// Total grade of the system (inflation grades from onlineStatus or multipGPU is counted)
    pub total: u64,
    /// Some Era，grade snap of stash account
    pub staker_statistic: BTreeMap<AccountId, StashMachineStatistics>,
}

/// Stash账户的统计
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct StashMachineStatistics {
    /// 用户在线的GPU数量
    pub online_gpu_num: u64,
    /// 用户对应的膨胀系数，由在线GPU数量决定
    pub inflation: Perbill,
    /// 用户的机器的总计算点数得分(不考虑膨胀)
    pub machine_total_calc_point: u64,
    /// 用户机器因被租用获得的额外得分
    pub rent_extra_grade: u64,
}

// 每台机器的基础得分与租用情况
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug, Clone)]
pub struct MachineGradeStatus<AccountId> {
    /// 机器的基础得分
    pub basic_grade: u64,
    /// 机器的租用状态
    pub is_rented: bool,
    /// 奖励的委员会
    pub reward_account: Vec<AccountId>,
}

impl<AccountId> EraStashPoints<AccountId>
where
    AccountId: Ord + Clone,
{
    /// 增加一台在线的机器，gpu数量 + gpu的总得分
    /// NOTE: 只修改当前Era，调用下线逻辑前应检查机器存在
    /// TODO: 还应该改变机器快照
    pub fn change_machine_online_status(
        &mut self,
        stash: AccountId,
        gpu_num: u64,
        basic_grade: u64,
        is_online: bool,
    ) {
        let mut staker_statistic = self
            .staker_statistic
            .entry(stash.clone())
            .or_insert(StashMachineStatistics { ..Default::default() });

        let old_grade = staker_statistic.total_grades().unwrap();

        if is_online {
            staker_statistic.online_gpu_num += gpu_num;
        } else {
            staker_statistic.online_gpu_num -= gpu_num;
        }

        // 根据显卡数量n更新inflation系数: inflation = min(10%, n/10000)
        // 当stash账户显卡数量n=1000时，inflation最大为10%
        staker_statistic.inflation = if staker_statistic.online_gpu_num <= 1000 {
            Perbill::from_rational_approximation(staker_statistic.online_gpu_num, 10_000)
        } else {
            Perbill::from_rational_approximation(1000u64, 10_000)
        };

        // 根据在线情况更改stash的基础分
        if is_online {
            staker_statistic.machine_total_calc_point += basic_grade;
        } else {
            staker_statistic.machine_total_calc_point -= basic_grade;
        }

        // 更新系统分数记录
        let new_grade = staker_statistic.total_grades().unwrap();
        self.total += new_grade;
        self.total -= old_grade;

        // 更新该stash账户的记录
        if staker_statistic.online_gpu_num == 0 {
            self.staker_statistic.remove(&stash);
        } else {
            let staker_statistic = (*staker_statistic).clone();
            self.staker_statistic.insert(stash, staker_statistic);
        }
    }

    /// 因机器租用状态改变，而影响得分
    pub fn change_machine_rent_status(
        &mut self,
        stash: AccountId,
        basic_grade: u64,
        is_rented: bool,
    ) {
        let mut staker_statistic = self
            .staker_statistic
            .entry(stash.clone())
            .or_insert(StashMachineStatistics { ..Default::default() });

        // 因租用而产生的分数
        let grade_by_rent = Perbill::from_rational_approximation(30u64, 100u64) * basic_grade;

        // 更新rent_extra_grade
        if is_rented {
            self.total += grade_by_rent;
            staker_statistic.rent_extra_grade += grade_by_rent;
        } else {
            self.total -= grade_by_rent;
            staker_statistic.rent_extra_grade -= grade_by_rent;
        }

        let staker_statistic = (*staker_statistic).clone();
        self.staker_statistic.insert(stash, staker_statistic);
    }
}

impl StashMachineStatistics {
    /// 该Stash账户对应的总得分
    /// total_grades = inflation * total_calc_point + total_calc_point + rent_grade
    pub fn total_grades(&self) -> Option<u64> {
        (self.inflation * self.machine_total_calc_point)
            .checked_add(self.machine_total_calc_point)?
            .checked_add(self.rent_extra_grade)
    }
}

impl<AccountId> MachineGradeStatus<AccountId> {
    pub fn machine_actual_grade(&self, inflation: Perbill) -> u64 {
        let rent_extra_grade = if self.is_rented {
            Perbill::from_rational_approximation(30u32, 100u32) * self.basic_grade
        } else {
            0
        };
        let inflation_extra_grade = inflation * self.basic_grade;
        self.basic_grade + rent_extra_grade + inflation_extra_grade
    }
}
