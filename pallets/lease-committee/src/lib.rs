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
    traits::{Currency, LockIdentifier, LockableCurrency, WithdrawReasons},
};
use frame_system::{ensure_root, ensure_signed, pallet_prelude::*};
use online_profile::CommitteeUploadInfo;
use online_profile_machine::LCOps;
use sp_runtime::{
    traits::{CheckedAdd, CheckedDiv, CheckedMul, CheckedSub, SaturatedConversion},
    RuntimeDebug,
};
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
use sp_std::{prelude::*, str, vec::Vec};

mod rpc_types;
pub use rpc_types::RpcLCCommitteeOps;

pub type MachineId = Vec<u8>;
pub type EraIndex = u32;
type BalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

pub const PALLET_LOCK_ID: LockIdentifier = *b"leasecom";
pub const DISTRIBUTION: u32 = 9; // 分成9个区间进行验证
                                 // pub const DURATIONPERCOMMITTEE: u32 = 480; // 每个用户有480个块的时间验证机器: 480 * 30 / 3600 = 4 hours

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

// 记录处于不同状态的委员会的列表，方便派单
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct LCCommitteeList<AccountId: Ord> {
    pub committee: Vec<AccountId>,    // 质押并通过社区选举的委员会
    pub chill_list: Vec<AccountId>,   // 委员会，但不想被派单
    pub fulfill_list: Vec<AccountId>, // 委员会, 但需要补交质押
    pub black_list: Vec<AccountId>,   // 委员会，黑名单中
}

impl<AccountId: Ord> LCCommitteeList<AccountId> {
    fn staker_exist(&self, who: &AccountId) -> bool {
        if let Ok(_) = self.committee.binary_search(who) {
            return true;
        }
        if let Ok(_) = self.chill_list.binary_search(who) {
            return true;
        }
        if let Ok(_) = self.fulfill_list.binary_search(who) {
            return true;
        }
        if let Ok(_) = self.black_list.binary_search(who) {
            return true;
        }
        false
    }

    fn add_staker(a_field: &mut Vec<AccountId>, new_staker: AccountId) {
        if let Err(index) = a_field.binary_search(&new_staker) {
            a_field.insert(index, new_staker);
        }
    }

    fn rm_staker(a_field: &mut Vec<AccountId>, drop_staker: &AccountId) {
        if let Ok(index) = a_field.binary_search(drop_staker) {
            a_field.remove(index);
        }
    }
}

// 从用户地址查询绑定的机器列表
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct LCCommitteeMachineList {
    pub booked_machine: Vec<MachineId>, // 记录分配给用户的机器ID及开始验证时间
    pub hashed_machine: Vec<MachineId>, // 存储已经提交了Hash信息的机器
    pub confirmed_machine: Vec<MachineId>, // 存储已经提交了原始确认数据的机器
    pub online_machine: Vec<MachineId>, // 存储已经成功上线的机器
}

// 一台机器对应的委员会
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct LCMachineCommitteeList<AccountId, BlockNumber> {
    pub book_time: BlockNumber,
    pub booked_committee: Vec<AccountId>, // 记录分配给机器的委员会及验证开始时间
    pub hashed_committee: Vec<AccountId>,
    pub confirm_start: BlockNumber, // 开始提交raw信息的时间
    pub confirmed_committee: Vec<AccountId>,
    pub onlined_committee: Vec<AccountId>, // 可以获得该机器在线奖励的委员会
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct LCCommitteeOps<BlockNumber, Balance> {
    pub booked_time: BlockNumber,
    pub staked_dbc: Balance,
    pub verify_time: Vec<BlockNumber>,
    pub confirm_hash: [u8; 16],
    pub hash_time: BlockNumber,
    pub confirm_time: BlockNumber, // 委员会提交raw信息的时间
    pub machine_status: MachineStatus,
    pub machine_info: CommitteeUploadInfo,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub enum MachineStatus {
    Booked,
    Hashed,
    Confirmed,
}

enum MachineConfirmStatus<AccountId> {
    Confirmed(Vec<AccountId>, CommitteeUploadInfo),
    Refuse(Vec<AccountId>, MachineId),
    NoConsensus,
}

impl Default for MachineStatus {
    fn default() -> Self {
        MachineStatus::Booked
    }
}

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config + generic_func::Config + dbc_price_ocw::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: LockableCurrency<Self::AccountId, Moment = Self::BlockNumber>;
        type LCOperations: LCOps<AccountId = Self::AccountId, MachineId = MachineId, CommitteeUploadInfo = CommitteeUploadInfo>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_finalize(_block_number: T::BlockNumber) {
            // 每个块高检查委员会质押是否足够
            if let Some(stake_dbc_amount) = Self::stake_dbc_amount() {
                CommitteeStakeDBCPerOrder::<T>::put(stake_dbc_amount);
            }

            Self::distribute_machines(); // 分派机器
            Self::statistic_result(); // 检查订单状态
        }
    }

    // 每次订单质押默认100RMB等价DBC
    #[pallet::storage]
    #[pallet::getter(fn committee_stake_usd_per_order)]
    pub(super) type CommitteeStakeUSDPerOrder<T: Config> = StorageValue<_, u64, ValueQuery>;

    // 每次订单默认质押等价的DBC数量
    #[pallet::storage]
    #[pallet::getter(fn committee_stake_dbc_per_order)]
    pub(super) type CommitteeStakeDBCPerOrder<T: Config> = StorageValue<_, BalanceOf<T>>;

    #[pallet::storage]
    #[pallet::getter(fn committee_total_stake)]
    pub(super) type CommitteeTotalStake<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>>;

    #[pallet::storage]
    #[pallet::getter(fn committee)]
    pub(super) type Committee<T: Config> = StorageValue<_, LCCommitteeList<T::AccountId>, ValueQuery>;

    // 存储用户订阅的不同确认阶段的机器
    #[pallet::storage]
    #[pallet::getter(fn committee_machine)]
    pub(super) type CommitteeMachine<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, LCCommitteeMachineList, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn machine_committee)]
    pub(super) type MachineCommittee<T: Config> = StorageMap<_, Blake2_128Concat, MachineId, LCMachineCommitteeList<T::AccountId, T::BlockNumber>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn committee_ops)]
    pub(super) type CommitteeOps<T: Config> = StorageDoubleMap<_, Blake2_128Concat, T::AccountId, Blake2_128Concat, MachineId, LCCommitteeOps<T::BlockNumber, BalanceOf<T>>, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // 设置committee每次验证需要质押数量, 单位为usd * 10^6
        #[pallet::weight(0)]
        pub fn set_staked_usd_per_order(origin: OriginFor<T>, value: u64) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            CommitteeStakeUSDPerOrder::<T>::put(value);
            Ok(().into())
        }

        // 该操作由社区决定
        // 添加到委员会，直接添加到fulfill列表中。每次finalize将会读取委员会币数量，币足则放到committee中
        #[pallet::weight(0)]
        pub fn add_committee(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut staker = Self::committee();
            // 确保用户还未加入到本模块
            ensure!(!staker.staker_exist(&member), Error::<T>::AccountAlreadyExist);
            // 将用户添加到fulfill列表中
            LCCommitteeList::add_staker(&mut staker.fulfill_list, member.clone());

            Committee::<T>::put(staker);
            Self::deposit_event(Event::CommitteeAdded(member));
            Ok(().into())
        }

        // 委员会停止接单
        #[pallet::weight(10000)]
        pub fn chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut staker = Self::committee();
            ensure!(staker.staker_exist(&who), Error::<T>::AccountNotExist);

            if let Ok(_) = staker.chill_list.binary_search(&who) {
                return Err(Error::<T>::AlreadyChill.into());
            }
            if let Ok(_) = staker.black_list.binary_search(&who) {
                return Err(Error::<T>::AlreadyInBlackList.into());
            }

            LCCommitteeList::rm_staker(&mut staker.committee, &who);
            LCCommitteeList::rm_staker(&mut staker.committee, &who);
            LCCommitteeList::add_staker(&mut staker.chill_list, who.clone());

            Committee::<T>::put(staker);
            Self::deposit_event(Event::Chill(who));

            Ok(().into())
        }

        // 委员会可以接单
        #[pallet::weight(10000)]
        pub fn undo_chill(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut staker = Self::committee();
            if let Err(_) = staker.chill_list.binary_search(&who) {
                return Err(Error::<T>::NotInChillList.into());
            }

            LCCommitteeList::rm_staker(&mut staker.chill_list, &who);
            LCCommitteeList::add_staker(&mut staker.fulfill_list, who.clone());
            Committee::<T>::put(staker);

            Self::deposit_event(Event::UndoChill(who));
            Ok(().into())
        }

        // 委员会可以退出, 从chill_list中退出
        #[pallet::weight(10000)]
        pub fn exit_staker(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;

            let mut staker = Self::committee();
            ensure!(staker.staker_exist(&who), Error::<T>::AccountNotExist);

            // 如果有未完成的工作，则不允许退出
            let committee_machines = Self::committee_machine(&who);
            if committee_machines.booked_machine.len() > 0 ||
                committee_machines.hashed_machine.len() > 0 ||
                committee_machines.confirmed_machine.len() > 0 {
                    return Err(Error::<T>::JobNotDone.into());
            }

            // 如果是candidacy，则可以直接退出, 从staker中删除
            // 如果是fulfill_list则可以直接退出(低于5wDBC的将进入fulfill_list，无法抢单,每次惩罚1w)
            LCCommitteeList::rm_staker(&mut staker.committee, &who);
            LCCommitteeList::rm_staker(&mut staker.fulfill_list, &who);
            LCCommitteeList::rm_staker(&mut staker.chill_list, &who);

            Committee::<T>::put(staker);

            Self::deposit_event(Event::ExitFromCandidacy(who));

            return Ok(().into());
        }

        #[pallet::weight(0)]
        pub fn add_black_list(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let mut staker = Self::committee();
            if staker.staker_exist(&member) {
                LCCommitteeList::rm_staker(&mut staker.committee, &member);
                LCCommitteeList::rm_staker(&mut staker.chill_list, &member);
                LCCommitteeList::rm_staker(&mut staker.fulfill_list, &member);
            }

            LCCommitteeList::add_staker(&mut staker.black_list, member.clone());
            Committee::<T>::put(staker);

            Self::deposit_event(Event::AddToBlackList(member));
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn rm_black_list(origin: OriginFor<T>, member: T::AccountId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;

            let mut staker = Self::committee();
            // 只从black_list中移出，想要加入委员会，需要社区再次投票
            LCCommitteeList::rm_staker(&mut staker.black_list, &member);

            Self::deposit_event(Event::RmFromBlackList(member));
            Ok(().into())
        }

        // 添加确认hash
        // FIXME: 注意，提交Hash需要检查，不与其他人的/已存在的Hash相同, 否则, 是很严重的作弊行为
        #[pallet::weight(10000)]
        fn add_confirm_hash(origin: OriginFor<T>, machine_id: MachineId, hash: [u8; 16]) -> DispatchResultWithPostInfo {
            let who = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut machine_committee = Self::machine_committee(&machine_id);

            // 从机器信息列表中有该委员会
            if let Err(_) = machine_committee.booked_committee.binary_search(&who) {
                return Err(Error::<T>::NotInBookList.into());
            }

            // 在该机器信息中，记录上委员的Hash
            if let Err(index) = machine_committee.hashed_committee.binary_search(&who) {
                machine_committee.hashed_committee.insert(index, who.clone());
            }

            // 从委员的任务中，删除该机器的任务
            let mut committee_machine = Self::committee_machine(&who);
            if let Ok(index) = committee_machine.booked_machine.binary_search(&machine_id) {
                committee_machine.booked_machine.remove(index);
            }

            // 委员会hashedmachine添加上该机器
            if let Err(index) = committee_machine.hashed_machine.binary_search(&machine_id) {
                committee_machine.hashed_machine.insert(index, machine_id.clone());
            }

            // 添加用户对机器的操作记录
            let mut committee_ops = Self::committee_ops(&who, &machine_id);
            committee_ops.machine_status = MachineStatus::Hashed;
            committee_ops.confirm_hash = hash.clone();
            committee_ops.hash_time = now;

            // 更新存储
            MachineCommittee::<T>::insert(&machine_id, machine_committee);
            CommitteeMachine::<T>::insert(&who, committee_machine);
            CommitteeOps::<T>::insert(&who, &machine_id, committee_ops);

            Self::deposit_event(Event::AddConfirmHash(who, hash));

            Ok(().into())
        }

        // fn submit_confirm_raw(origin: OriginFor<T>, machine_id: MachineId, confirm_raw: Vec<u8>) -> DispatchResultWithPostInfo {
        // 委员会提交的原始信息
        #[pallet::weight(10000)]
        fn submit_confirm_raw(
            origin: OriginFor<T>, machine_info_detail: CommitteeUploadInfo) -> DispatchResultWithPostInfo
        {
            let who = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let machine_id = machine_info_detail.machine_id.clone();

            let mut machine_committee = Self::machine_committee(&machine_id);
            let mut committee_machine = Self::committee_machine(&who);
            let mut machine_ops = Self::committee_ops(&who, &machine_id);

            // 查询是否已经到了提交hash的时间 必须在36 ~ 48小时之间
            ensure!(now >= machine_committee.confirm_start, Error::<T>::TimeNotAllow);
            ensure!(now <= machine_committee.book_time + (3600u32 / 30 * 48).into(), Error::<T>::TimeNotAllow);

            // 该用户已经给机器提交过Hash
            if let Err(_) = machine_committee.hashed_committee.binary_search(&who) {
                return Err(Error::<T>::NotSubmitHash.into());
            }

            // 机器ID存在于用户已经Hash的机器里
            if let Err(_) = committee_machine.hashed_machine.binary_search(&machine_id) {
                return Err(Error::<T>::NotSubmitHash.into());
            }

            // 检查提交的raw与已提交的Hash一致
            let info_hash = machine_info_detail.hash();
            ensure!(info_hash == machine_ops.confirm_hash, Error::<T>::NotAllHashSubmited);

            // 用户还未提交过原始信息
            if let Ok(_) = committee_machine.confirmed_machine.binary_search(&machine_id) {
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
            machine_ops.machine_status = MachineStatus::Confirmed;
            machine_ops.machine_info = machine_info_detail.clone();

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
        StakeToBeCandidacy(T::AccountId, BalanceOf<T>),
        CommitteeAdded(T::AccountId),
        CommitteeRemoved(T::AccountId),
        CandidacyAdded(T::AccountId),
        CandidacyRemoved(T::AccountId),
        Chill(T::AccountId),
        UndoChill(T::AccountId),
        Withdrawn(T::AccountId, BalanceOf<T>),
        AddToWhiteList(T::AccountId),
        AddToBlackList(T::AccountId),
        ExitFromCandidacy(T::AccountId),
        CommitteeFulfill(BalanceOf<T>),
        AddConfirmHash(T::AccountId, [u8; 16]),
        RmFromBlackList(T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        AccountAlreadyExist,
        AlreadyChill,
        AccountNotExist,
        CandidacyLimitReached,
        AlreadyCandidacy,
        NotCandidacy,
        CommitteeLimitReached,
        AlreadyCommittee,
        NotCommittee,
        MachineGradePriceNotSet,
        CommitteeConfirmedYet,
        StakeNotEnough,
        FreeBalanceNotEnough,
        AlreadyInChillList,
        UserInBlackList,
        AlreadyInWhiteList,
        NotInWhiteList,
        UserInWhiteList,
        AlreadyInBlackList,
        NotInBlackList,
        NotInBookList,
        BookFailed,
        AlreadySubmitHash,
        StatusNotAllowedSubmitHash,
        NotAllHashSubmited,
        HashIsNotIdentical,
        NoMachineCanBook,
        NoMachineIdFound,
        JobNotDone,
        NotHealthStatus,
        NoNeedFulfill,
        NoLedgerFound,
        MinStakeNotFound,
        NotInChillList,
        TimeNotAllow,
        NotSubmitHash,
        AlreadySubmitRaw,
    }
}

impl<T: Config> Pallet<T> {
    // 根据DBC价格获得需要质押数量
    fn stake_dbc_amount() -> Option<BalanceOf<T>> {
        let dbc_price: BalanceOf<T> = <dbc_price_ocw::Module<T>>::avg_price()?.saturated_into();
        let one_dbc: BalanceOf<T> = 1000_000_000_000_000u64.saturated_into();
        let committee_stake_need: BalanceOf<T> = Self::committee_stake_usd_per_order().saturated_into();

        one_dbc.checked_mul(&committee_stake_need)?.checked_div(&dbc_price)
    }

    // 检查委员会是否有足够的质押
    // 在每个区块以及每次分配一个机器之后，都需要检查
    fn check_committee_free_balance() -> Result<(), ()> {
        let mut committee = Self::committee();
        let stake_per_gpu = Self::committee_stake_dbc_per_order().ok_or(())?;

        let current_committee = committee.committee.clone();
        let current_fulfill_list = committee.fulfill_list.clone();

        // 如果free_balance不够，则移动到fulfill_list中
        for a_committee in current_committee {
            // 当委员会质押不够时，将委员会移动到fulfill_list中
            if <T as Config>::Currency::free_balance(&a_committee) < stake_per_gpu {
                if let Ok(index) = committee.committee.binary_search(&a_committee) {
                    committee.committee.remove(index);
                    if let Err(index) = committee.fulfill_list.binary_search(&a_committee) {
                        committee.fulfill_list.insert(index, a_committee);
                    }
                }
            }
        }
        // 如果free_balance够，则移动到正常状态中
        for a_committee in current_fulfill_list {
            if <T as Config>::Currency::free_balance(&a_committee) > stake_per_gpu {
                if let Ok(index) = committee.fulfill_list.binary_search(&a_committee) {
                    committee.fulfill_list.remove(index);
                    if let Err(index) = committee.committee.binary_search(&a_committee) {
                        committee.committee.insert(index, a_committee);
                    }
                }
            }
        }

        Committee::<T>::put(committee);
        return Ok(())
    }

    // 获取所有新加入的机器，并进行分派给委员会
    fn distribute_machines() {
        let live_machines = <online_profile::Pallet<T>>::live_machines();
        for a_machine_id in live_machines.ocw_confirmed_machine {
            debug::warn!("#### distribute machine: {:?}", &a_machine_id);
            let _ = Self::distribute_one_machine(&a_machine_id);
        }
    }

    fn distribute_one_machine(machine_id: &MachineId) -> Result<(), ()> {
        let lucky_committee = Self::lucky_committee().ok_or(())?;

        debug::warn!("#### lucky_committee: {:?} for machine: {:?}", &lucky_committee, machine_id);

        // 每个添加4个小时
        let now = <frame_system::Module<T>>::block_number();
        let confirm_start = now + (3600u32 / 30 * 36).into(); // 添加确认信息时间为分发之后的36小时

        for a_book in lucky_committee {
            let _ = Self::book_one(machine_id.to_vec(), confirm_start, now, a_book);
        }

        // 将机器状态从ocw_confirmed_machine改为booked_machine
        T::LCOperations::lc_booked_machine(machine_id.clone());
        Ok(())
    }

    // 一个委员会进行操作
    fn book_one(machine_id: MachineId, confirm_start: T::BlockNumber,
                now: T::BlockNumber,order_time: (T::AccountId, Vec<usize>)) -> Result<(), ()> {

        // 增加质押
        let stake_need = Self::committee_stake_dbc_per_order().ok_or(())?;
        debug::warn!("#### Stake need: {:?}", &stake_need);

        Self::add_stake(&order_time.0, stake_need)?;

        debug::warn!("#### will change following status");

        // 修改machine对应的委员会
        let mut machine_committee = Self::machine_committee(&machine_id);
        machine_committee.book_time = now;
        if let Err(index) = machine_committee.booked_committee.binary_search(&order_time.0) {
            machine_committee.booked_committee.insert(index, order_time.0.clone());
        }
        machine_committee.confirm_start = confirm_start;

        // 修改委员会对应的machine
        let mut committee_machine = Self::committee_machine(&order_time.0);
        if let Err(index) = committee_machine.booked_machine.binary_search(&machine_id) {
            committee_machine.booked_machine.insert(index, machine_id.clone());
        }

        // 修改委员会的操作
        let mut committee_ops = Self::committee_ops(&order_time.0, &machine_id);
        committee_ops.booked_time = now;
        committee_ops.staked_dbc = stake_need;
        let start_time: Vec<_> = (0..order_time.1.len()).map(|x| now + (x as u32 * 3600u32 / 30 * 4).into()).collect();
        committee_ops.verify_time = start_time;
        committee_ops.machine_status = MachineStatus::Booked;

        // 存储变量
        MachineCommittee::<T>::insert(&machine_id, machine_committee);
        CommitteeMachine::<T>::insert(&order_time.0, committee_machine);
        CommitteeOps::<T>::insert(&order_time.0, &machine_id, committee_ops);

        Ok(())
    }

    // 分派一个machineId给随机的委员会
    // 返回Distribution(9)个随机顺序的账户列表
    fn lucky_committee() -> Option<Vec<(T::AccountId, Vec<usize>)>> {
        // 检查质押数量如果有委员会质押数量不够，则重新获取lucky_committee
        Self::check_committee_free_balance().ok()?;

        let staker = Self::committee();
        let mut committee = staker.committee.clone();
        // 如果委员会数量为0，直接返回空列表
        if committee.len() == 0 {
            return None;
        }

        // 有多少个幸运的委员会： min(staker.committee.len(), 3)
        let lucky_committee_num = committee.len().min(3);

        // 选出lucky_committee_num个委员会
        let mut lucky_committee = Vec::new();

        for _ in 0..lucky_committee_num {
            let lucky_index = <generic_func::Module<T>>::random_u32(committee.len() as u32 - 1u32) as usize;
            lucky_committee.push((committee[lucky_index].clone(), Vec::new()));
            committee.remove(lucky_index);
        }

        for i in 0..DISTRIBUTION as usize {
            let index = i % lucky_committee_num;
            lucky_committee[index].1.push(i);
        }

        Some(lucky_committee)
    }

    fn statistic_result() {
        let live_machines = <online_profile::Pallet<T>>::live_machines();
        let booked_machine = live_machines.booked_machine;
        let now = <frame_system::Module<T>>::block_number();

        for machine_id in booked_machine {
            // 如果机器超过了48个小时，则查看：
            // 是否有委员会提交了确认信息
            let machine_committee = Self::machine_committee(machine_id.clone());
            if now < machine_committee.book_time + (3600u32 / 30 * 48).into() {
                return;
            }

            match Self::summary_confirmation(&machine_id) {
                MachineConfirmStatus::Confirmed(committee, machine_info) => {
                    let _ = T::LCOperations::lc_confirm_machine(committee, machine_info);
                },
                MachineConfirmStatus::Refuse(committee, machine_id) => {
                    // 如果是委员会判定失败，则扣除所有奖金
                    let _ = T::LCOperations::lc_refuse_machine(committee, machine_id);
                },
                MachineConfirmStatus::NoConsensus => {
                    // 没有委员会添加确认信息，或者意见相反委员会相等, 则进行新一轮评估
                    let _ = Self::revert_book(machine_id.clone());
                },
            }
        }
    }

    // 重新进行派单评估
    // 该函数将清除本模块信息，并将online_profile机器状态改为ocw_confirmed_machine
    // 清除信息： LCCommitteeMachineList, LCMachineCommitteeList, LCCommitteeOps
    fn revert_book(machine_id: MachineId) -> Result<(),()> {
        // 查询质押，并退还质押
        T::LCOperations::lc_revert_booked_machine(machine_id.clone());

        let machine_committee = Self::machine_committee(&machine_id);
        for booked_committee in machine_committee.booked_committee {
            let committee_ops = Self::committee_ops(&booked_committee, &machine_id);
            Self::reduce_stake(&booked_committee, committee_ops.staked_dbc)?;

            CommitteeOps::<T>::remove(&booked_committee, &machine_id);

            let mut committee_machine = Self::committee_machine(&booked_committee);
            if let Ok(index) = committee_machine.booked_machine.binary_search(&machine_id) {
                committee_machine.booked_machine.remove(index);
            }
            if let Ok(index) = committee_machine.hashed_machine.binary_search(&machine_id) {
                committee_machine.hashed_machine.remove(index);
            }
            if let Ok(index) = committee_machine
                .confirmed_machine
                .binary_search(&machine_id)
            {
                committee_machine.confirmed_machine.remove(index);
            }
            CommitteeMachine::<T>::insert(booked_committee, committee_machine);
        }

        MachineCommittee::<T>::remove(&machine_id);
        Ok(())
    }

    // 总结机器的确认情况
    // 检查机器是否被确认，并检查提交的信息是否一致
    // 返回三种情况：1. 认可，则添加机器; 2. 不认可，则退出机器； 3. 没达成共识
    fn summary_confirmation(machine_id: &MachineId) -> MachineConfirmStatus<T::AccountId> {
        let machine_committee = Self::machine_committee(machine_id);

        let mut against = 0usize;
        let mut against_committee = Vec::new();

        let mut uniq_machine_info: Vec<CommitteeUploadInfo> = Vec::new();
        let mut committee_for_machine_info = Vec::new();

        if machine_committee.confirmed_committee.len() == 0 {
            return MachineConfirmStatus::NoConsensus;
        }

        for a_committee in machine_committee.confirmed_committee {
            let a_machine_info = Self::committee_ops(a_committee.clone(), machine_id).machine_info;

            // 如果该委员会反对该机器
            if a_machine_info.is_support == false {
                against_committee.push(a_committee);
                against += 1;
                continue
            }

            match uniq_machine_info.iter().position(|r| r == &a_machine_info){
                None => {
                    uniq_machine_info.push(a_machine_info.clone());
                    committee_for_machine_info.push(vec![a_committee.clone()]);
                },
                Some(index) => {
                    committee_for_machine_info[index].push(a_committee)
                }
            };
        }

        // 统计committee_for_machine_info中有多少委员会站队最多
        let support_committee_num: Vec<usize> = committee_for_machine_info.iter().map(|item| item.len()).collect();
        let max_support = support_committee_num.iter().max(); // 最多多少个委员会达成一致意见

        match max_support {
            None => {
                if against > 0{
                    return MachineConfirmStatus::Refuse(against_committee, machine_id.to_vec());
                }
                return MachineConfirmStatus::NoConsensus;
            },
            Some(max_support_num) => {
                let max_support_group = support_committee_num.iter().filter(|n| n == &max_support_num).count();

                if max_support_group == 1 {
                    let committee_group_index = support_committee_num.iter().position(|r| r == max_support_num).unwrap();
                    let support_committee = committee_for_machine_info[committee_group_index].clone();

                    if against > max_support_group {
                        return MachineConfirmStatus::Refuse(support_committee, machine_id.to_vec());
                    }
                    if against == max_support_group {
                        return MachineConfirmStatus::NoConsensus;
                    }

                    return MachineConfirmStatus::Confirmed(support_committee, uniq_machine_info[committee_group_index].clone());
                }

                // 否则，max_support_group > 1
                if against > *max_support_num {
                    return MachineConfirmStatus::Refuse(against_committee, machine_id.to_vec());
                }
                // against == max_support 或者 against < max_support 时，都是无法达成共识
                return MachineConfirmStatus::NoConsensus;
            }
        }
    }

    fn add_stake(controller: &T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        let total_stake = Self::committee_total_stake(&controller).unwrap_or(0u32.into());
        let new_stake = total_stake.checked_add(&amount).ok_or(())?;

        <T as Config>::Currency::set_lock(PALLET_LOCK_ID, controller, new_stake, WithdrawReasons::all());
        Ok(())
    }

    fn reduce_stake(controller: &T::AccountId, amount: BalanceOf<T>) -> Result<(), ()> {
        let total_stake = Self::committee_total_stake(&controller).ok_or(())?;
        let new_stake = total_stake.checked_sub(&amount).ok_or(())?;

        <T as Config>::Currency::set_lock(PALLET_LOCK_ID, controller, new_stake, WithdrawReasons::all());
        Ok(())
    }
}

// RPC
impl<T: Config> Module<T> {
    pub fn get_sum() -> u64 {
        return 3
    }

    pub fn get_committee_machine_list(committee: T::AccountId) -> LCCommitteeMachineList {
        Self::committee_machine(committee)
    }

    pub fn get_committee_ops(committee: T::AccountId, machine_id: MachineId) -> RpcLCCommitteeOps<T::BlockNumber, BalanceOf<T>> {
        let lc_committee_ops = Self::committee_ops(&committee, &machine_id);

        RpcLCCommitteeOps {
            booked_time: lc_committee_ops.booked_time,
            staked_dbc: lc_committee_ops.staked_dbc,
            // pub verify_time: Vec<BlockNumber>, // FIXME: return Vec<BlockNumber> type
            confirm_hash: lc_committee_ops.confirm_hash,
            hash_time: lc_committee_ops.hash_time,
            confirm_time: lc_committee_ops.confirm_time,
            machine_status: lc_committee_ops.machine_status,
            machine_info: lc_committee_ops.machine_info,
        }
    }
}
