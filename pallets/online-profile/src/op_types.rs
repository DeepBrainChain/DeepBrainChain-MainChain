use codec::{alloc::string::ToString, Decode, Encode, HasCompact};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_io::hashing::blake2_128;
use sp_runtime::{
    traits::{AtLeast32BitUnsigned, Saturating, Zero},
    RuntimeDebug,
};
use sp_std::{collections::btree_map::BTreeMap, collections::vec_deque::VecDeque, prelude::*};

pub type MachineId = Vec<u8>;
pub type EraIndex = u32;
pub type ImageName = Vec<u8>;

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
    pub hard_disk: u64,    // 硬盘
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
        let hard_disk: Vec<u8> = self.hard_disk.to_string().into();
        let cpu_core_num: Vec<u8> = self.cpu_core_num.to_string().into();
        let cpu_rate: Vec<u8> = self.cpu_rate.to_string().into();
        let mem_num: Vec<u8> = self.mem_num.to_string().into();

        let is_support: Vec<u8> = if self.is_support {
            "true".into()
        } else {
            "false".into()
        };

        let mut raw_info = Vec::new();
        raw_info.extend(self.machine_id.clone());
        raw_info.extend(self.gpu_type.clone());
        raw_info.extend(gpu_num);
        raw_info.extend(cuda_core);
        raw_info.extend(gpu_mem);
        raw_info.extend(calc_point);
        raw_info.extend(hard_disk);
        raw_info.extend(self.cpu_type.clone());
        raw_info.extend(cpu_core_num);
        raw_info.extend(cpu_rate);
        raw_info.extend(mem_num);

        raw_info.extend(self.rand_str.clone());
        raw_info.extend(is_support);

        return blake2_128(&raw_info);
    }
}

// 不确定值，由机器管理者提交
#[derive(Debug, PartialEq, Eq, Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct StakerCustomizeInfo {
    pub left_change_time: u64, // 用户对贷款及经纬度的修改次数

    pub upload_net: u64,   // 上行带宽
    pub download_net: u64, // 下行带宽
    pub longitude: u64,    // 经度
    pub latitude: u64,     // 纬度

    pub images: Vec<ImageName>, // 镜像名称
}

impl Default for StakerCustomizeInfo {
    fn default() -> Self {
        StakerCustomizeInfo {
            left_change_time: 3,
            upload_net: 0,   // 不确定值, 存储平均值
            download_net: 0, // 不确定值, 存储平均值
            longitude: 0,    // 经度, 不确定值，存储平均值
            latitude: 0,     // 纬度, 不确定值，存储平均值
            images: Vec::new(),
        }
    }
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct OPStakingLedger<AccountId, Balance: HasCompact> {
    pub stash: AccountId,

    #[codec(compact)]
    pub total: Balance,

    #[codec(compact)]
    pub active: Balance,

    pub unlocking: Vec<UnlockChunk<Balance>>,
    pub claimed_rewards: Vec<EraIndex>,

    pub released_rewards: Balance, // 委员会和用户已经释放的奖励
    pub upcoming_rewards: VecDeque<Balance>, // 用户剩余未释放的奖励
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
pub struct UnlockChunk<Balance: HasCompact> {
    #[codec(compact)]
    pub value: Balance,

    #[codec(compact)]
    pub era: EraIndex,
}

// 记录每个Era的机器的总分
#[derive(PartialEq, Encode, Decode, Default, RuntimeDebug)]
pub struct EraMachinePoints {
    // 所有可以奖励的机器总得分
    pub total: u64,
    // 某个Era，所有可以奖励的机器
    pub individual: BTreeMap<MachineId, u64>,
}

impl<AccountId, Balance: HasCompact + Copy + Saturating + AtLeast32BitUnsigned>
    OPStakingLedger<AccountId, Balance>
{
    // 筛选去掉已经到期的unlocking
    pub fn consolidate_unlock(self, current_era: EraIndex) -> Self {
        let mut total = self.total;
        let unlocking = self
            .unlocking
            .into_iter()
            .filter(|chunk| {
                if chunk.era > current_era {
                    true
                } else {
                    total = total.saturating_sub(chunk.value);
                    false
                }
            })
            .collect();

        Self {
            stash: self.stash,
            total,
            active: self.active,
            unlocking,
            claimed_rewards: self.claimed_rewards,
            released_rewards: self.released_rewards,
            upcoming_rewards: self.upcoming_rewards,
        }
    }
}

impl<AccountId, Balance> OPStakingLedger<AccountId, Balance>
where
    Balance: AtLeast32BitUnsigned + Saturating + Copy,
{
    pub fn slash(&mut self, mut value: Balance, minimum_balance: Balance) -> Balance {
        let pre_total = self.total;
        let total = &mut self.total; // total = active + releasing
        let active = &mut self.active;

        let slash_out_of =
            |total_remaining: &mut Balance, target: &mut Balance, value: &mut Balance| {
                let mut slash_from_target = (*value).min(*target); // 最小惩罚 = min(avtive, slash)

                if !slash_from_target.is_zero() {
                    *target -= slash_from_target;

                    if *target <= minimum_balance {
                        slash_from_target += *target;
                        *value += sp_std::mem::replace(target, Zero::zero());
                    }

                    *total_remaining = total_remaining.saturating_sub(slash_from_target);
                    *value -= slash_from_target;
                }
            };

        slash_out_of(total, active, &mut value); // 扣除处罚的资金

        let i = self
            .unlocking
            .iter_mut()
            .map(|chunk| {
                slash_out_of(total, &mut chunk.value, &mut value); // 从正在解压的部分中，扣除剩下的罚款
                chunk.value
            })
            .take_while(|value| value.is_zero())
            .count();

        let _ = self.unlocking.drain(..i); // 删掉为0的chunk

        pre_total.saturating_sub(*total) // 返回一共惩罚成功的资金
    }
}
