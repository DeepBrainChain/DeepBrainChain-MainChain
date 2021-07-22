// 委员会不设置个数限制，满足质押，并且通过议案选举即可。
// 每次机器加入，系统随机选择3个委员会，对应到0~36h。每次验证区间为4个小时,共有9个验证区间。
// 每个委员随机分得3个验证区间，进行验证。
// 下一轮选择，与上一轮委员会是否被选择的状态无关。
// 委员会确认机器，会提供三个字段组成的 Hash1 = Hash(机器原始信息, 委员会随机字符串, bool(机器正常与否))

// Hash(GPU型号, GPU数量, CUDA core数量, GPU显存, 算力值, 硬盘, 上行带宽, 下行带宽, CPU型号, CPU内核数)

// 最后12个小时，统计委员会结果，多数结果为最终结果。第二次提交信息为： 机器原始信息，委员会随机字符串，bool.
// 验证：1. Hash(机器原始信息) == OCW获取到的机器Hash
//      2. Hash(机器原始信息，委员会随机字符串, bool) == Hash1
// 如果没有人提交信息，则进行新一轮随机派发。

// 成功上线，则退还委员会质押

// 钱包地址：xxxx
// 钱包签名信息：xxxx
#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{
    ensure,
    pallet_prelude::*,
    traits::{Currency, LockableCurrency},
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use online_profile::CommitteeUploadInfo;
use online_profile_machine::{LCOps, ManageCommittee};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_runtime::RuntimeDebug;
use sp_std::{prelude::*, str, vec::Vec};

mod rpc_types;
pub use rpc_types::RpcLCCommitteeOps;

pub type MachineId = Vec<u8>;
pub type EraIndex = u32;
type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub const DISTRIBUTION: u32 = 9; // 分成9个区间进行验证

pub const DURATIONPERCOMMITTEE: u32 = 480; // 每个用户有480个块的时间验证机器: 480 * 30 / 3600 = 4 hours
pub const SUBMIT_RAW_START: u32 = 4320; // 在分派之后的36个小时后允许提交原始信息
pub const SUBMIT_RAW_END: u32 = 5760; // 在分派之后的48小时总结

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// 从用户地址查询绑定的机器列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct LCCommitteeMachineList {
    pub booked_machine: Vec<MachineId>, // 记录分配给用户的机器ID及开始验证时间
    pub hashed_machine: Vec<MachineId>, // 存储已经提交了Hash信息的机器
    pub confirmed_machine: Vec<MachineId>, // 存储已经提交了原始确认数据的机器
    pub online_machine: Vec<MachineId>, // 存储已经成功上线的机器
}

// 机器对应的验证委员会
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub struct LCMachineCommitteeList<AccountId, BlockNumber> {
    pub book_time: BlockNumber,              // 系统分派订单的时间
    pub booked_committee: Vec<AccountId>,    // 订单分配的委员会
    pub hashed_committee: Vec<AccountId>,    // 提交了Hash的委员会列表
    pub confirm_start_time: BlockNumber,     // 系统设定的开始提交raw信息的委员会
    pub confirmed_committee: Vec<AccountId>, // 已经提交了原始信息的委员会
    pub onlined_committee: Vec<AccountId>,   // 若机器成功上线，可以获得该机器在线奖励的委员会
    pub status: LCVerifyStatus,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum LCVerifyStatus {
    SubmittingHash,
    SubmittingRaw,
    Summarizing,
    Finished,
}

impl Default for LCVerifyStatus {
    fn default() -> Self {
        LCVerifyStatus::SubmittingHash
    }
}

// 一个委员会对对机器的操作记录
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct LCCommitteeOps<BlockNumber, Balance> {
    pub staked_dbc: Balance,
    pub verify_time: Vec<BlockNumber>, // 委员会可以验证机器的时间
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    pub confirm_time: BlockNumber, // 委员会提交raw信息的时间
    pub machine_status: LCMachineStatus,
    pub machine_info: CommitteeUploadInfo,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "std", serde(rename_all = "camelCase"))]
pub enum LCMachineStatus {
    Booked,
    Hashed,
    Confirmed,
}

impl Default for LCMachineStatus {
    fn default() -> Self {
        LCMachineStatus::Booked
    }
}

// 委员会完成提交信息后，可能会出现的情况
enum MachineConfirmStatus<AccountId> {
    Confirmed(Summary<AccountId>),   // 支持的委员会，反对的委员会，机器信息
    Refuse(Summary<AccountId>),      // 支持的委员会，反对的委员会，机器信息
    NoConsensus(Summary<AccountId>), // 如果由于没有委员会提交信息而无共识，则委员会将受到惩罚
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
struct Summary<AccountId> {
    pub valid_support: Vec<AccountId>,   // 有效的支持者
    pub invalid_support: Vec<AccountId>, // 无效的支持者
    pub unruly: Vec<AccountId>,          // 没有提交全部信息的委员会
    pub against: Vec<AccountId>,
    pub info: Option<CommitteeUploadInfo>,
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + online_profile::Config + generic_func::Config + committee::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type LCOperations: LCOps<
            AccountId = Self::AccountId,
            MachineId = MachineId,
            CommitteeUploadInfo = CommitteeUploadInfo,
        >;
        type ManageCommittee: ManageCommittee<
            AccountId = Self::AccountId,
            BalanceOf = BalanceOf<Self>,
        >;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_block_number: T::BlockNumber) {
            Self::distribute_machines(); // 分派机器
            Self::statistic_result(); // 检查订单状态
        }
    }

    // 存储用户订阅的不同确认阶段的机器
    #[pallet::storage]
    #[pallet::getter(fn committee_machine)]
    pub(super) type CommitteeMachine<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, LCCommitteeMachineList, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn machine_committee)]
    pub(super) type MachineCommittee<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        LCMachineCommitteeList<T::AccountId, T::BlockNumber>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn committee_ops)]
    pub(super) type CommitteeOps<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        MachineId,
        LCCommitteeOps<T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 添加确认hash
        #[pallet::weight(10000)]
        pub fn submit_confirm_hash(
            origin: OriginFor<T>,
            machine_id: MachineId,
            hash: [u8; 16],
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            debug::error!("####### Hash is: {:?}", hash);

            let mut machine_committee = Self::machine_committee(&machine_id);

            // 从机器信息列表中有该委员会
            machine_committee
                .booked_committee
                .binary_search(&who)
                .map_err(|_| Error::<T>::NotInBookList)?;

            // 该委员会没有提交过Hash
            if machine_committee.hashed_committee.binary_search(&who).is_ok() {
                return Err(Error::<T>::AlreadySubmitHash.into());
            }

            // 检查该Hash未出现过
            for a_committee in machine_committee.hashed_committee.clone() {
                let machine_ops = Self::committee_ops(&a_committee, &machine_id);
                if machine_ops.confirm_hash == hash {
                    // 与其中一个委员会提交的Hash一致
                    // FIXME: 注意，提交Hash需要检查，不与其他人的/已存在的Hash相同, 否则将被认为是作弊行为
                    // Self::revert_book(machine_id)
                }
            }

            // 在该机器信息中，记录上委员的Hash
            if let Err(index) = machine_committee.hashed_committee.binary_search(&who) {
                machine_committee.hashed_committee.insert(index, who.clone());
            }

            let mut committee_machine = Self::committee_machine(&who);

            // 从委员的任务中，删除该机器的任务
            if let Ok(index) = committee_machine.booked_machine.binary_search(&machine_id) {
                committee_machine.booked_machine.remove(index);
            }
            // 委员会hashedmachine添加上该机器
            if let Err(index) = committee_machine.hashed_machine.binary_search(&machine_id) {
                committee_machine.hashed_machine.insert(index, machine_id.clone());
            }

            // 添加用户对机器的操作记录
            let mut committee_ops = Self::committee_ops(&who, &machine_id);
            committee_ops.machine_status = LCMachineStatus::Hashed;
            committee_ops.confirm_hash = hash.clone();
            committee_ops.hash_time = now;

            // 如果委员会都提交了Hash,则直接进入提交原始信息的阶段
            if machine_committee.booked_committee.len() == machine_committee.hashed_committee.len()
            {
                machine_committee.status = LCVerifyStatus::SubmittingRaw;
            }

            // 更新存储
            MachineCommittee::<T>::insert(&machine_id, machine_committee);
            CommitteeMachine::<T>::insert(&who, committee_machine);
            CommitteeOps::<T>::insert(&who, &machine_id, committee_ops);

            Self::deposit_event(Event::AddConfirmHash(who, hash));

            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn change_machine_hash(
            origin: OriginFor<T>,
            committee: T::AccountId,
            machine_id: MachineId,
            new_hash: [u8; 16],
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let mut committee_ops = Self::committee_ops(&committee, &machine_id);
            committee_ops.confirm_hash = new_hash;
            CommitteeOps::<T>::insert(committee, machine_id, committee_ops);
            Ok(().into())
        }

        /// 委员会提交的原始信息
        #[pallet::weight(10000)]
        pub fn submit_confirm_raw(
            origin: OriginFor<T>,
            machine_info_detail: CommitteeUploadInfo,
        ) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let machine_id = machine_info_detail.machine_id.clone();

            let mut machine_committee = Self::machine_committee(&machine_id);
            let mut committee_machine = Self::committee_machine(&who);
            let mut machine_ops = Self::committee_ops(&who, &machine_id);

            // 如果所有人都提交了，则直接可以提交Hash
            if machine_committee.status != LCVerifyStatus::SubmittingRaw {
                // 查询是否已经到了提交hash的时间 必须在36 ~ 48小时之间
                ensure!(now >= machine_committee.confirm_start_time, Error::<T>::TimeNotAllow);
                ensure!(
                    now <= machine_committee.book_time + SUBMIT_RAW_END.into(),
                    Error::<T>::TimeNotAllow
                );
            }

            // 该用户已经给机器提交过Hash
            machine_committee
                .hashed_committee
                .binary_search(&who)
                .map_err(|_| Error::<T>::NotSubmitHash)?;

            // 机器ID存在于用户已经Hash的机器里
            committee_machine
                .hashed_machine
                .binary_search(&machine_id)
                .map_err(|_| Error::<T>::NotSubmitHash)?;

            // 检查提交的raw与已提交的Hash一致
            let info_hash = machine_info_detail.hash();
            ensure!(info_hash == machine_ops.confirm_hash, Error::<T>::NotAllHashSubmited);

            // 用户还未提交过原始信息
            if committee_machine.confirmed_machine.binary_search(&machine_id).is_ok() {
                return Err(Error::<T>::AlreadySubmitRaw.into());
            }

            // 修改存储
            if let Ok(index) = committee_machine.hashed_machine.binary_search(&machine_id) {
                committee_machine.hashed_machine.remove(index);
            }
            if let Err(index) = committee_machine.confirmed_machine.binary_search(&machine_id) {
                committee_machine.confirmed_machine.insert(index, machine_id.clone());
            }

            if let Err(index) = machine_committee.confirmed_committee.binary_search(&who) {
                machine_committee.confirmed_committee.insert(index, who.clone());
            }

            // machine_ops.confirm_raw = confirm_raw.clone();
            machine_ops.confirm_time = now;
            machine_ops.machine_status = LCMachineStatus::Confirmed;
            machine_ops.machine_info = machine_info_detail.clone();
            machine_ops.machine_info.rand_str = Vec::new();

            // 如果全部都提交完了原始信息，则允许进入summary
            if machine_committee.confirmed_committee.len()
                == machine_committee.booked_committee.len()
            {
                machine_committee.status = LCVerifyStatus::Summarizing;
            }

            CommitteeMachine::<T>::insert(&who, committee_machine);
            MachineCommittee::<T>::insert(&machine_id, machine_committee);
            CommitteeOps::<T>::insert(&who, &machine_id, machine_ops);

            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AddConfirmHash(T::AccountId, [u8; 16]),
    }

    #[pallet::error]
    pub enum Error<T> {
        NotInBookList,
        AlreadySubmitHash,
        NotAllHashSubmited,
        TimeNotAllow,
        NotSubmitHash,
        AlreadySubmitRaw,
    }
}

impl<T: Config> Pallet<T> {
    // 获取所有新加入的机器，并进行分派给委员会
    pub fn distribute_machines() {
        let live_machines = <online_profile::Pallet<T>>::live_machines();
        for a_machine_id in live_machines.confirmed_machine {
            debug::warn!("Distribute machine: {:?}", &a_machine_id);
            let _ = Self::distribute_one_machine(&a_machine_id);
        }
    }

    pub fn distribute_one_machine(machine_id: &MachineId) -> Result<(), ()> {
        let lucky_committee = Self::lucky_committee().ok_or(())?;

        debug::warn!("Lucky committee: {:?} for machine: {:?}", &lucky_committee, machine_id);

        // 每个添加4个小时
        let now = <frame_system::Module<T>>::block_number();
        let confirm_start = now + SUBMIT_RAW_START.into(); // 添加确认信息时间为分发之后的36小时

        for a_book in lucky_committee {
            let _ = Self::book_one(machine_id.to_vec(), confirm_start, now, a_book);
        }

        // 将机器状态从ocw_confirmed_machine改为booked_machine
        T::LCOperations::lc_booked_machine(machine_id.clone());
        Ok(())
    }

    // 一个委员会进行操作
    fn book_one(
        machine_id: MachineId,
        confirm_start: T::BlockNumber,
        now: T::BlockNumber,
        order_time: (T::AccountId, Vec<usize>),
    ) -> Result<(), ()> {
        // 增加质押：由committee执行
        let stake_need = <T as pallet::Config>::ManageCommittee::stake_per_order().ok_or(())?;
        <T as pallet::Config>::ManageCommittee::change_stake(&order_time.0, stake_need, true)?;

        debug::warn!("Will change following status");

        // 修改machine对应的委员会
        let mut machine_committee = Self::machine_committee(&machine_id);
        machine_committee.book_time = now;
        if let Err(index) = machine_committee.booked_committee.binary_search(&order_time.0) {
            machine_committee.booked_committee.insert(index, order_time.0.clone());
        }
        machine_committee.confirm_start_time = confirm_start;

        // 修改委员会对应的machine
        let mut committee_machine = Self::committee_machine(&order_time.0);
        if let Err(index) = committee_machine.booked_machine.binary_search(&machine_id) {
            committee_machine.booked_machine.insert(index, machine_id.clone());
        }

        // 修改委员会的操作
        let mut committee_ops = Self::committee_ops(&order_time.0, &machine_id);
        committee_ops.staked_dbc = stake_need;
        let start_time: Vec<_> = order_time
            .1
            .into_iter()
            .map(|x| now + (x as u32 * SUBMIT_RAW_START / DISTRIBUTION).into())
            .collect();
        committee_ops.verify_time = start_time;
        committee_ops.machine_status = LCMachineStatus::Booked;

        // 存储变量
        MachineCommittee::<T>::insert(&machine_id, machine_committee);
        CommitteeMachine::<T>::insert(&order_time.0, committee_machine);
        CommitteeOps::<T>::insert(&order_time.0, &machine_id, committee_ops);

        Ok(())
    }

    // 分派一个machineId给随机的委员会
    // 返回Distribution(9)个随机顺序的账户列表
    pub fn lucky_committee() -> Option<Vec<(T::AccountId, Vec<usize>)>> {
        let mut committee = <committee::Module<T>>::available_committee().ok()?;

        // 如果委员会数量为0，直接返回空列表
        if committee.len() == 0 {
            debug::warn!("No available committee found");
            return None;
        }

        // 有多少个幸运的委员会： min(staker.committee.len(), 3)
        let lucky_committee_num = committee.len().min(3);

        // 选出lucky_committee_num个委员会
        let mut lucky_committee = Vec::new();

        for _ in 0..lucky_committee_num {
            let lucky_index =
                <generic_func::Module<T>>::random_u32(committee.len() as u32 - 1u32) as usize;
            lucky_committee.push((committee[lucky_index].clone(), Vec::new()));
            committee.remove(lucky_index);
        }

        for i in 0..DISTRIBUTION as usize {
            let index = i % lucky_committee_num;
            lucky_committee[index].1.push(i);
        }

        Some(lucky_committee)
    }

    pub fn statistic_result() {
        let live_machines = <online_profile::Pallet<T>>::live_machines();
        let booked_machine = live_machines.booked_machine;
        let now = <frame_system::Module<T>>::block_number();

        for machine_id in booked_machine {
            let machine_committee = Self::machine_committee(machine_id.clone());
            // 当不为Summary状态时查看是否到了48小时，如果不到则返回
            if machine_committee.status != LCVerifyStatus::Summarizing {
                if now < machine_committee.book_time + SUBMIT_RAW_END.into() {
                    continue;
                }
            }

            let mut slash_committee = Vec::new(); // 应该被惩罚的委员会
            let mut reward_committee = Vec::new(); // 当拒绝上线时，惩罚委员会的币奖励给拒绝的委员会
            let mut unstake_committee = Vec::new(); // 解除质押的委员会

            debug::warn!("Summarying... {:?}", machine_id);

            match Self::summary_confirmation(&machine_id) {
                MachineConfirmStatus::Confirmed(summary) => {
                    debug::warn!("Summarying result is... confirmed");
                    slash_committee.extend(summary.unruly.clone());
                    slash_committee.extend(summary.against);
                    slash_committee.extend(summary.invalid_support);
                    unstake_committee.extend(summary.valid_support.clone());
                    if let Ok(_) = T::LCOperations::lc_confirm_machine(
                        summary.valid_support.clone(),
                        summary.info.unwrap(),
                    ) {
                        let valid_support = summary.valid_support.clone();
                        for a_committee in valid_support {
                            let mut committee_machine = Self::committee_machine(&a_committee);
                            if let Ok(index) =
                                committee_machine.confirmed_machine.binary_search(&machine_id)
                            {
                                committee_machine.confirmed_machine.remove(index);
                            }
                            if let Err(index) =
                                committee_machine.online_machine.binary_search(&machine_id)
                            {
                                committee_machine.online_machine.insert(index, machine_id.clone());
                            }
                            CommitteeMachine::<T>::insert(&a_committee, committee_machine);
                        }

                        let mut machine_committee = Self::machine_committee(&machine_id);
                        machine_committee.status = LCVerifyStatus::Finished;
                        machine_committee.onlined_committee = summary.valid_support;
                    }
                }
                MachineConfirmStatus::Refuse(summary) => {
                    debug::warn!("Summarying result is... refused");
                    slash_committee.extend(summary.unruly.clone());
                    slash_committee.extend(summary.invalid_support);
                    reward_committee.extend(summary.against.clone());
                    unstake_committee.extend(summary.against.clone());

                    if let Err(e) = T::LCOperations::lc_refuse_machine(machine_id.clone()) {
                        debug::error!("Failed to exec lc refuse machine logic: {:?}", e);
                    };
                }
                MachineConfirmStatus::NoConsensus(summary) => {
                    debug::warn!("Summarying result is... NoConsensus");
                    slash_committee.extend(summary.unruly.clone());
                    unstake_committee.extend(machine_committee.confirmed_committee.clone());
                    if let Err(e) = Self::revert_book(machine_id.clone()) {
                        debug::error!("Failed to revert book: {:?}", e);
                    };

                    T::LCOperations::lc_revert_booked_machine(machine_id.clone());
                }
            }

            // 惩罚没有提交信息的委员会
            for a_committee in slash_committee {
                let committee_ops = Self::committee_ops(&a_committee, &machine_id);
                <T as pallet::Config>::ManageCommittee::add_slash(
                    a_committee,
                    committee_ops.staked_dbc,
                    vec![],
                );
                // TODO: 应该从book的信息中移除
            }

            for a_committee in unstake_committee {
                let committee_ops = Self::committee_ops(&a_committee, &machine_id);
                if let Err(e) = <T as pallet::Config>::ManageCommittee::change_stake(
                    &a_committee,
                    committee_ops.staked_dbc,
                    false,
                ) {
                    debug::error!("Change stake of {:?} failed: {:?}", &a_committee, e);
                };
            }
        }
    }

    fn _clean_book(machine_id: MachineId, committee: T::AccountId) {
        CommitteeOps::<T>::remove(&committee, &machine_id);

        let mut committee_machine = Self::committee_machine(&committee);
        if let Ok(index) = committee_machine.booked_machine.binary_search(&machine_id) {
            committee_machine.booked_machine.remove(index);
        }
        if let Ok(index) = committee_machine.hashed_machine.binary_search(&machine_id) {
            committee_machine.hashed_machine.remove(index);
        }
        if let Ok(index) = committee_machine.confirmed_machine.binary_search(&machine_id) {
            committee_machine.confirmed_machine.remove(index);
        }
        CommitteeMachine::<T>::insert(committee, committee_machine);
    }

    // 重新进行派单评估
    // 该函数将清除本模块信息，并将online_profile机器状态改为ocw_confirmed_machine
    // 清除信息： LCCommitteeMachineList, LCMachineCommitteeList, LCCommitteeOps
    fn revert_book(machine_id: MachineId) -> Result<(), ()> {
        let machine_committee = Self::machine_committee(&machine_id);

        // 给提交了信息的委员会退押金
        for booked_committee in machine_committee.confirmed_committee {
            let _committee_ops = Self::committee_ops(&booked_committee, &machine_id);
            // TODO: committee 提供
            // Self::reduce_stake(&booked_committee, committee_ops.staked_dbc)?;
        }

        // 清除预订了机器的委员会
        for booked_committee in machine_committee.booked_committee {
            CommitteeOps::<T>::remove(&booked_committee, &machine_id);

            let mut committee_machine = Self::committee_machine(&booked_committee);
            if let Ok(index) = committee_machine.booked_machine.binary_search(&machine_id) {
                committee_machine.booked_machine.remove(index);
            }
            if let Ok(index) = committee_machine.hashed_machine.binary_search(&machine_id) {
                committee_machine.hashed_machine.remove(index);
            }
            if let Ok(index) = committee_machine.confirmed_machine.binary_search(&machine_id) {
                committee_machine.confirmed_machine.remove(index);
            }
            CommitteeMachine::<T>::insert(booked_committee, committee_machine);
        }

        MachineCommittee::<T>::remove(&machine_id);
        Ok(())
    }

    // 总结机器的确认情况: 检查机器是否被确认，并检查提交的信息是否一致
    // 返回三种状态：
    // 1. 无共识：处理办法：退还委员会质押，机器重新派单。
    // 2. 支持上线: 处理办法：扣除所有反对上线，支持上线但提交无效信息的委员会的质押。
    // 3. 反对上线: 处理办法：反对的委员会平分支持的委员会的质押。扣5%矿工质押，允许矿工再次质押而上线。
    fn summary_confirmation(machine_id: &MachineId) -> MachineConfirmStatus<T::AccountId> {
        let machine_committee = Self::machine_committee(machine_id);

        let mut summary = Summary { ..Default::default() };

        let mut uniq_machine_info: Vec<CommitteeUploadInfo> = Vec::new(); // 支持的委员会可能提交不同的机器信息
        let mut committee_for_machine_info = Vec::new(); // 不同机器信息对应的委员会

        for a_committee in machine_committee.booked_committee {
            // 记录没有提交原始信息的委员会
            if machine_committee.confirmed_committee.binary_search(&a_committee).is_err() {
                summary.unruly.push(a_committee);
                continue;
            }

            let a_machine_info = Self::committee_ops(a_committee.clone(), machine_id).machine_info;
            // 记录上反对上线的委员会
            if a_machine_info.is_support == false {
                summary.against.push(a_committee);
                continue;
            }

            match uniq_machine_info.iter().position(|r| r == &a_machine_info) {
                None => {
                    uniq_machine_info.push(a_machine_info.clone());
                    committee_for_machine_info.push(vec![a_committee.clone()]);
                }
                Some(index) => committee_for_machine_info[index].push(a_committee),
            };
        }

        // 如果没有人提交确认信息，则无共识。返回分派了订单的委员会列表，对其进行惩罚
        if machine_committee.confirmed_committee.len() == 0 {
            return MachineConfirmStatus::NoConsensus(summary);
        }

        // 统计committee_for_machine_info中有多少委员会站队最多
        let support_committee_num: Vec<usize> =
            committee_for_machine_info.iter().map(|item| item.len()).collect();
        let max_support = support_committee_num.iter().max(); // 最多多少个委员会达成一致意见

        match max_support {
            None => {
                // 如果没有支持者，且有反对者，则拒绝接入。
                if summary.against.len() > 0 {
                    return MachineConfirmStatus::Refuse(summary);
                }
                // 反对者支持者都为0
                return MachineConfirmStatus::NoConsensus(summary);
            }
            Some(max_support_num) => {
                // 多少个机器信息的支持等于最大的支持
                let max_support_group =
                    support_committee_num.iter().filter(|n| n == &max_support_num).count();

                if max_support_group == 1 {
                    let committee_group_index =
                        support_committee_num.iter().position(|r| r == max_support_num).unwrap();

                    // 记录所有的无效支持
                    for index in 0..committee_for_machine_info.len() {
                        if index == committee_group_index {
                            continue;
                        }
                        summary.invalid_support.extend(committee_for_machine_info[index].clone());
                    }
                    // 记录上所有的有效支持
                    summary.valid_support =
                        committee_for_machine_info[committee_group_index].clone();

                    if summary.against.len() > max_support_group {
                        // 反对多于支持
                        return MachineConfirmStatus::Refuse(summary);
                    } else if summary.against.len() == max_support_group {
                        // 反对等于支持
                        return MachineConfirmStatus::NoConsensus(summary);
                    } else {
                        // 反对小于支持
                        summary.info = Some(uniq_machine_info[committee_group_index].clone());
                        return MachineConfirmStatus::Confirmed(summary);
                    }
                }

                // 如果有两组都是Max个委员会支, 则所有的支持都是无效的支持
                for index in 0..committee_for_machine_info.len() {
                    summary.invalid_support.extend(committee_for_machine_info[index].clone())
                }
                if summary.against.len() > *max_support_num {
                    return MachineConfirmStatus::Refuse(summary);
                }

                // against <= max_support 且 max_support_group > 1，且反对的不占多数
                return MachineConfirmStatus::NoConsensus(summary);
            }
        }
    }
}

// RPC
impl<T: Config> Module<T> {
    pub fn get_machine_committee_list(
        machine_id: MachineId,
    ) -> LCMachineCommitteeList<T::AccountId, T::BlockNumber> {
        Self::machine_committee(machine_id)
    }

    pub fn get_committee_machine_list(committee: T::AccountId) -> LCCommitteeMachineList {
        Self::committee_machine(committee)
    }

    pub fn get_committee_ops(
        committee: T::AccountId,
        machine_id: MachineId,
    ) -> RpcLCCommitteeOps<T::BlockNumber, BalanceOf<T>> {
        let lc_committee_ops = Self::committee_ops(&committee, &machine_id);
        let committee_info = Self::machine_committee(&machine_id);

        RpcLCCommitteeOps {
            booked_time: committee_info.book_time,
            staked_dbc: lc_committee_ops.staked_dbc,
            verify_time: lc_committee_ops.verify_time,
            confirm_hash: lc_committee_ops.confirm_hash,
            hash_time: lc_committee_ops.hash_time,
            confirm_time: lc_committee_ops.confirm_time,
            machine_status: lc_committee_ops.machine_status,
            machine_info: lc_committee_ops.machine_info,
        }
    }
}
