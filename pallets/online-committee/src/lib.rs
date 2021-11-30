#![cfg_attr(not(feature = "std"), no_std)]

pub mod rpc;
mod slash;
mod types;

#[cfg(test)]
mod mock;
#[cfg(test)]
mod tests;

use frame_support::{
    ensure,
    pallet_prelude::*,
    traits::{Currency, ReservableCurrency},
};
use frame_system::{ensure_signed, pallet_prelude::*};
use generic_func::{ItemList, MachineId, SlashId};
use online_profile::CommitteeUploadInfo;
use online_profile_machine::{GNOps, ManageCommittee, OCOps};
use sp_std::{prelude::*, str, vec::Vec};

pub use pallet::*;
pub use types::*;

type BalanceOf<T> = <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + online_profile::Config + generic_func::Config + committee::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type OCOperations: OCOps<
            AccountId = Self::AccountId,
            MachineId = MachineId,
            CommitteeUploadInfo = CommitteeUploadInfo,
            Balance = BalanceOf<Self>,
        >;
        type ManageCommittee: ManageCommittee<AccountId = Self::AccountId, Balance = BalanceOf<Self>>;
        type CancelSlashOrigin: EnsureOrigin<Self::Origin>;
        type SlashAndReward: GNOps<AccountId = Self::AccountId, Balance = BalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_n: BlockNumberFor<T>) -> frame_support::weights::Weight {
            Self::check_and_exec_pending_review();
            Self::check_and_exec_pending_slash();
            0
        }

        fn on_finalize(_block_number: T::BlockNumber) {
            Self::statistic_result();
            Self::distribute_machines();
        }
    }

    // 存储用户订阅的不同确认阶段的机器
    #[pallet::storage]
    #[pallet::getter(fn committee_machine)]
    pub(super) type CommitteeMachine<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, OCCommitteeMachineList, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn machine_committee)]
    pub(super) type MachineCommittee<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, OCMachineCommitteeList<T::AccountId, T::BlockNumber>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn machine_submited_hash)]
    pub(super) type MachineSubmitedHash<T> = StorageMap<_, Blake2_128Concat, MachineId, Vec<[u8; 16]>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn committee_ops)]
    pub(super) type CommitteeOps<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        MachineId,
        OCCommitteeOps<T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn next_slash_id)]
    pub(super) type NextSlashId<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pending_slash)]
    pub(super) type PendingSlash<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        SlashId,
        OCPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn pending_slash_review)]
    pub(super) type PendingSlashReview<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        SlashId,
        OCPendingSlashReviewInfo<T::AccountId, BalanceOf<T>, T::BlockNumber>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn unhandled_slash)]
    pub(super) type UnhandledSlash<T: Config> = StorageValue<_, Vec<SlashId>, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::weight(10000)]
        pub fn submit_confirm_hash(
            origin: OriginFor<T>,
            machine_id: MachineId,
            hash: [u8; 16],
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut machine_committee = Self::machine_committee(&machine_id);
            let mut machine_submited_hash = Self::machine_submited_hash(&machine_id);

            ensure!(machine_committee.booked_committee.binary_search(&committee).is_ok(), Error::<T>::NotInBookList);
            ensure!(
                machine_committee.hashed_committee.binary_search(&committee).is_err(),
                Error::<T>::AlreadySubmitHash
            );
            ensure!(machine_submited_hash.binary_search(&hash).is_err(), Error::<T>::DuplicateHash);
            ItemList::add_item(&mut machine_submited_hash, hash.clone());

            let mut committee_ops = Self::committee_ops(&committee, &machine_id);
            let mut committee_machine = Self::committee_machine(&committee);

            ItemList::add_item(&mut machine_committee.hashed_committee, committee.clone());
            ItemList::rm_item(&mut committee_machine.booked_machine, &machine_id);
            ItemList::add_item(&mut committee_machine.hashed_machine, machine_id.clone());

            // 添加用户对机器的操作记录
            committee_ops.machine_status = OCMachineStatus::Hashed;
            committee_ops.confirm_hash = hash.clone();
            committee_ops.hash_time = now;

            // 如果委员会都提交了Hash,则直接进入提交原始信息的阶段
            if machine_committee.booked_committee.len() == machine_committee.hashed_committee.len() {
                machine_committee.status = OCVerifyStatus::SubmittingRaw;
            }

            // 更新存储
            MachineSubmitedHash::<T>::insert(&machine_id, machine_submited_hash);
            MachineCommittee::<T>::insert(&machine_id, machine_committee);
            CommitteeMachine::<T>::insert(&committee, committee_machine);
            CommitteeOps::<T>::insert(&committee, &machine_id, committee_ops);

            Self::deposit_event(Event::AddConfirmHash(committee, hash));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn submit_confirm_raw(
            origin: OriginFor<T>,
            machine_info_detail: CommitteeUploadInfo,
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let machine_id = machine_info_detail.machine_id.clone();

            let mut machine_committee = Self::machine_committee(&machine_id);
            let mut committee_machine = Self::committee_machine(&committee);
            let mut machine_ops = Self::committee_ops(&committee, &machine_id);

            if machine_committee.status != OCVerifyStatus::SubmittingRaw {
                ensure!(now >= machine_committee.confirm_start_time, Error::<T>::TimeNotAllow);
                ensure!(now <= machine_committee.book_time + SUBMIT_RAW_END.into(), Error::<T>::TimeNotAllow);
            }
            ensure!(machine_committee.hashed_committee.binary_search(&committee).is_ok(), Error::<T>::NotSubmitHash);
            ensure!(committee_machine.hashed_machine.binary_search(&machine_id).is_ok(), Error::<T>::NotSubmitHash);
            ensure!(
                committee_machine.confirmed_machine.binary_search(&machine_id).is_err(),
                Error::<T>::AlreadySubmitRaw
            );

            let info_hash = machine_info_detail.hash();
            ensure!(info_hash == machine_ops.confirm_hash, Error::<T>::InfoNotFeatHash);

            ItemList::rm_item(&mut committee_machine.hashed_machine, &machine_id);
            ItemList::add_item(&mut committee_machine.confirmed_machine, machine_id.clone());
            ItemList::add_item(&mut machine_committee.confirmed_committee, committee.clone());

            machine_ops.confirm_time = now;
            machine_ops.machine_status = OCMachineStatus::Confirmed;
            machine_ops.machine_info = machine_info_detail.clone();
            machine_ops.machine_info.rand_str = Vec::new();

            if machine_committee.confirmed_committee.len() == machine_committee.hashed_committee.len() {
                machine_committee.status = OCVerifyStatus::Summarizing;
            }

            CommitteeMachine::<T>::insert(&committee, committee_machine);
            MachineCommittee::<T>::insert(&machine_id, machine_committee);
            CommitteeOps::<T>::insert(&committee, &machine_id, machine_ops);

            Self::deposit_event(Event::AddConfirmRaw(committee, machine_id));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn apply_slash_review(
            origin: OriginFor<T>,
            slash_id: SlashId,
            reason: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let applicant = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            ensure!(!PendingSlashReview::<T>::contains_key(slash_id), Error::<T>::AlreadyApplied);

            let committee_order_stake =
                <T as pallet::Config>::ManageCommittee::stake_per_order().ok_or(Error::<T>::GetStakeAmountFailed)?;

            let slash_info = Self::pending_slash(slash_id);

            let is_slashed_stash = match slash_info.book_result {
                OCBookResultType::OnlineRefused => &slash_info.machine_stash == &applicant,
                _ => false,
            };

            let is_slashed_committee = slash_info.inconsistent_committee.binary_search(&applicant).is_ok();

            ensure!(is_slashed_stash || is_slashed_committee, Error::<T>::NotSlashed);

            ensure!(
                <T as Config>::Currency::can_reserve(&applicant, committee_order_stake),
                Error::<T>::BalanceNotEnough
            );

            if is_slashed_stash {
                ensure!(
                    T::OCOperations::oc_change_staked_balance(applicant.clone(), committee_order_stake, true).is_ok(),
                    Error::<T>::BalanceNotEnough
                );
            } else {
                let _ =
                    <T as Config>::ManageCommittee::change_total_stake(applicant.clone(), committee_order_stake, true);
                let _ =
                    <T as Config>::ManageCommittee::change_used_stake(applicant.clone(), committee_order_stake, true);
            }

            <T as pallet::Config>::Currency::reserve(&applicant, committee_order_stake)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;

            PendingSlashReview::<T>::insert(
                slash_id,
                OCPendingSlashReviewInfo {
                    applicant,
                    staked_amount: committee_order_stake,
                    apply_time: now,
                    expire_time: slash_info.slash_exec_time,
                    reason,
                },
            );

            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn cancel_slash(origin: OriginFor<T>, slash_id: SlashId) -> DispatchResultWithPostInfo {
            <T as pallet::Config>::CancelSlashOrigin::ensure_origin(origin)?;

            Self::do_cancel_slash(slash_id)
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        AddConfirmHash(T::AccountId, [u8; 16]),
        AddConfirmRaw(T::AccountId, MachineId),
        MachineDistributed(MachineId, T::AccountId),
    }

    #[pallet::error]
    pub enum Error<T> {
        NotInBookList,
        AlreadySubmitHash,
        NotAllHashSubmited,
        TimeNotAllow,
        NotSubmitHash,
        AlreadySubmitRaw,
        InfoNotFeatHash,
        DuplicateHash,
        GetStakeAmountFailed,
        AlreadyApplied,
        NotSlashed,
        BalanceNotEnough,
        NotPendingReviewSlash,
        ExpiredApply,
    }
}

impl<T: Config> Pallet<T> {
    // 获取所有新加入的机器，并进行分派给委员会
    pub fn distribute_machines() {
        let live_machines = <online_profile::Pallet<T>>::live_machines();
        let now = <frame_system::Module<T>>::block_number();
        let confirm_start = now + SUBMIT_RAW_START.into();

        for a_machine_id in live_machines.confirmed_machine {
            // 重新分配: 必须清空该状态
            if MachineCommittee::<T>::contains_key(&a_machine_id) {
                MachineCommittee::<T>::remove(&a_machine_id);
            }

            if let Some(committee_workflow) = Self::committee_workflow() {
                for a_committee_workflow in committee_workflow {
                    if Self::book_one(a_machine_id.to_vec(), confirm_start, now, a_committee_workflow.clone()).is_err()
                    {
                        continue
                    };
                }
                // 将机器状态从ocw_confirmed_machine改为booked_machine
                T::OCOperations::oc_booked_machine(a_machine_id.clone());
            };
        }
    }

    // 分派一个machineId给随机的委员会
    // 返回Distribution(9)个随机顺序的账户列表
    pub fn committee_workflow() -> Option<Vec<(T::AccountId, Vec<usize>)>> {
        let mut committee = <committee::Module<T>>::available_committee()?;
        // Require committee_num at lease 3
        let lucky_committee_num = if committee.len() < 3 { return None } else { 3 };
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

    // 一个委员会进行操作
    // - Writes: MachineCommittee, CommitteeMachine, CommitteeOps
    fn book_one(
        machine_id: MachineId,
        confirm_start: T::BlockNumber,
        now: T::BlockNumber,
        work_time: (T::AccountId, Vec<usize>),
    ) -> Result<(), ()> {
        let stake_need = <T as pallet::Config>::ManageCommittee::stake_per_order().ok_or(())?;
        // Change committee usedstake will nerver fail after set proper params
        <T as pallet::Config>::ManageCommittee::change_used_stake(work_time.0.clone(), stake_need, true)?;

        // 修改machine对应的委员会
        let mut machine_committee = Self::machine_committee(&machine_id);

        ItemList::add_item(&mut machine_committee.booked_committee, work_time.0.clone());
        machine_committee.book_time = now;
        machine_committee.confirm_start_time = confirm_start;

        // 修改委员会对应的machine
        let mut committee_machine = Self::committee_machine(&work_time.0);
        ItemList::add_item(&mut committee_machine.booked_machine, machine_id.clone());

        // 修改委员会的操作
        let mut committee_ops = OCCommitteeOps { ..Default::default() };
        let start_time: Vec<_> =
            work_time.1.into_iter().map(|x| now + (x as u32 * SUBMIT_RAW_START / DISTRIBUTION).into()).collect();

        committee_ops.staked_dbc = stake_need;
        committee_ops.verify_time = start_time;
        committee_ops.machine_status = OCMachineStatus::Booked;

        // 存储变量
        MachineCommittee::<T>::insert(&machine_id, machine_committee);
        CommitteeMachine::<T>::insert(&work_time.0, committee_machine);
        CommitteeOps::<T>::insert(&work_time.0, &machine_id, committee_ops);

        Self::deposit_event(Event::MachineDistributed(machine_id.to_vec(), work_time.0));
        Ok(())
    }

    fn statistic_result() {
        let now = <frame_system::Module<T>>::block_number();
        let booked_machine = <online_profile::Pallet<T>>::live_machines().booked_machine;
        let committee_stake_per_order = <T as pallet::Config>::ManageCommittee::stake_per_order().unwrap_or_default();

        for machine_id in booked_machine {
            Self::statistic_a_machine(machine_id, now, committee_stake_per_order);
        }
    }

    fn statistic_a_machine(machine_id: MachineId, now: T::BlockNumber, committee_stake_per_order: BalanceOf<T>) {
        let mut machine_committee = Self::machine_committee(machine_id.clone());
        // 当不为Summary状态时查看是否到了48小时，则还需要继续等待
        if machine_committee.status != OCVerifyStatus::Summarizing &&
            now < machine_committee.book_time + SUBMIT_RAW_END.into()
        {
            return
        }

        let mut inconsistent_committee = Vec::new();
        let mut unruly_committee = Vec::new();
        let mut reward_committee = Vec::new();

        let mut book_result = OCBookResultType::OnlineSucceed;

        // type: (who, amount)
        let mut stash_slash_info = None;

        match Self::summary_confirmation(&machine_id) {
            MachineConfirmStatus::Confirmed(summary) => {
                unruly_committee = summary.unruly.clone();
                reward_committee = summary.valid_support.clone();

                for a_committee in summary.against {
                    ItemList::add_item(&mut inconsistent_committee, a_committee);
                }
                for a_committee in summary.invalid_support {
                    ItemList::add_item(&mut inconsistent_committee, a_committee);
                }

                if T::OCOperations::oc_confirm_machine(summary.valid_support.clone(), summary.info.unwrap()).is_ok() {
                    for a_committee in &summary.valid_support {
                        // 如果机器成功上线，则从委员会确认的机器中删除，添加到成功上线的记录中
                        let mut committee_machine = Self::committee_machine(a_committee);
                        ItemList::add_item(&mut committee_machine.online_machine, machine_id.clone());
                        CommitteeMachine::<T>::insert(a_committee, committee_machine);
                    }

                    machine_committee.status = OCVerifyStatus::Finished;
                    machine_committee.onlined_committee = summary.valid_support;
                }
            },
            MachineConfirmStatus::Refuse(summary) => {
                for a_committee in summary.unruly {
                    ItemList::add_item(&mut unruly_committee, a_committee);
                }
                for a_committee in summary.invalid_support {
                    ItemList::add_item(&mut inconsistent_committee, a_committee);
                }
                for a_committee in summary.against {
                    ItemList::add_item(&mut reward_committee, a_committee);
                }

                machine_committee.status = OCVerifyStatus::Finished;

                // should cancel machine_stash slash when slashed committee apply review
                stash_slash_info = T::OCOperations::oc_refuse_machine(machine_id.clone());

                book_result = OCBookResultType::OnlineRefused;
            },
            MachineConfirmStatus::NoConsensus(summary) => {
                for a_committee in summary.unruly {
                    ItemList::add_item(&mut unruly_committee, a_committee);
                }

                let _ = Self::revert_book(machine_id.clone());
                T::OCOperations::oc_revert_booked_machine(machine_id.clone());
                book_result = OCBookResultType::NoConsensus;
            },
        }

        MachineCommittee::<T>::insert(&machine_id, machine_committee.clone());

        if inconsistent_committee.len() == 0 && unruly_committee.len() == 0 {
            for a_committee in reward_committee {
                let _ = <T as pallet::Config>::ManageCommittee::change_used_stake(
                    a_committee,
                    committee_stake_per_order,
                    false,
                );
            }
        } else {
            // FIXME: cannot run here, when all committee refused
            let slash_id = Self::get_new_slash_id();
            let (machine_stash, stash_slash_amount) = stash_slash_info.unwrap_or_default();
            PendingSlash::<T>::insert(
                slash_id,
                OCPendingSlashInfo {
                    machine_id: machine_id.clone(),
                    machine_stash,
                    stash_slash_amount,

                    inconsistent_committee,
                    unruly_committee,
                    reward_committee,
                    committee_stake: committee_stake_per_order,

                    slash_time: now,
                    slash_exec_time: now + TWO_DAY.into(),

                    book_result,
                    slash_result: OCSlashResult::Pending,
                },
            );
            let mut unhandled_slash = Self::unhandled_slash();
            ItemList::add_item(&mut unhandled_slash, slash_id);
            UnhandledSlash::<T>::put(unhandled_slash);
        }

        // Do cleaning
        for a_committee in machine_committee.booked_committee {
            CommitteeOps::<T>::remove(&a_committee, &machine_id);
            MachineSubmitedHash::<T>::remove(&machine_id);

            // 改变committee_machine
            let mut committee_machine = Self::committee_machine(&a_committee);
            ItemList::rm_item(&mut committee_machine.booked_machine, &machine_id);
            ItemList::rm_item(&mut committee_machine.hashed_machine, &machine_id);
            ItemList::rm_item(&mut committee_machine.confirmed_machine, &machine_id);

            CommitteeMachine::<T>::insert(&a_committee, committee_machine);
        }
    }

    // 重新进行派单评估
    // 该函数将清除本模块信息，并将online_profile机器状态改为ocw_confirmed_machine
    // 清除信息： OCCommitteeMachineList, OCMachineCommitteeList, OCCommitteeOps
    fn revert_book(machine_id: MachineId) -> Result<(), ()> {
        let machine_committee = Self::machine_committee(&machine_id);

        // 清除预订了机器的委员会
        for booked_committee in machine_committee.booked_committee {
            CommitteeOps::<T>::remove(&booked_committee, &machine_id);

            let mut committee_machine = Self::committee_machine(&booked_committee);
            ItemList::rm_item(&mut committee_machine.booked_machine, &machine_id);
            ItemList::rm_item(&mut committee_machine.hashed_machine, &machine_id);
            ItemList::rm_item(&mut committee_machine.confirmed_machine, &machine_id);

            CommitteeMachine::<T>::insert(booked_committee, committee_machine);
        }

        MachineCommittee::<T>::remove(&machine_id);
        Ok(())
    }

    fn get_new_slash_id() -> u64 {
        let slash_id = Self::next_slash_id();

        if slash_id == u64::MAX {
            NextSlashId::<T>::put(0);
        } else {
            NextSlashId::<T>::put(slash_id + 1);
        };

        slash_id
    }

    // 总结机器的确认情况: 检查机器是否被确认，并检查提交的信息是否一致
    // 返回三种状态：
    // 1. 无共识：处理办法：退还委员会质押，机器重新派单。
    // 2. 支持上线: 处理办法：扣除所有反对上线，支持上线但提交无效信息的委员会的质押。
    // 3. 反对上线: 处理办法：反对的委员会平分支持的委员会的质押。扣5%矿工质押，允许矿工再次质押而上线。
    pub fn summary_confirmation(machine_id: &MachineId) -> MachineConfirmStatus<T::AccountId> {
        let machine_committee = Self::machine_committee(machine_id);

        let mut summary = Summary::default();
        // 支持的委员会可能提交不同的机器信息
        let mut uniq_machine_info: Vec<CommitteeUploadInfo> = Vec::new();
        // 不同机器信息对应的委员会
        let mut committee_for_machine_info = Vec::new();

        // 记录没有提交原始信息的委员会
        summary.unruly = machine_committee.summary_unruly();

        // 如果没有人提交确认信息，则无共识。返回分派了订单的委员会列表，对其进行惩罚
        if machine_committee.confirmed_committee.len() == 0 {
            return MachineConfirmStatus::NoConsensus(summary)
        }

        // 记录上反对上线的委员会
        for a_committee in machine_committee.confirmed_committee.clone() {
            let submit_machine_info = Self::committee_ops(a_committee.clone(), machine_id).machine_info;
            if !submit_machine_info.is_support {
                ItemList::add_item(&mut summary.against, a_committee);
            } else {
                match uniq_machine_info.iter().position(|r| r == &submit_machine_info) {
                    None => {
                        uniq_machine_info.push(submit_machine_info.clone());
                        committee_for_machine_info.push(vec![a_committee.clone()]);
                    },
                    Some(index) => ItemList::add_item(&mut committee_for_machine_info[index], a_committee),
                };
            }
        }

        // 统计committee_for_machine_info中有多少委员会站队最多
        let support_committee_num: Vec<usize> = committee_for_machine_info.iter().map(|item| item.len()).collect();
        // 最多多少个委员会达成一致意见
        let max_support = support_committee_num.clone().into_iter().max();
        if max_support.is_none() {
            // 如果没有支持者，且有反对者，则拒绝接入。
            if summary.against.len() > 0 {
                return MachineConfirmStatus::Refuse(summary)
            }
            // 反对者支持者都为0
            return MachineConfirmStatus::NoConsensus(summary)
        }

        let max_support_num = max_support.unwrap();

        // 多少个机器信息的支持等于最大的支持
        let max_support_group = support_committee_num.clone().into_iter().filter(|n| n == &max_support_num).count();

        if max_support_group == 1 {
            let committee_group_index =
                support_committee_num.clone().into_iter().position(|r| r == max_support_num).unwrap();

            // 记录所有的无效支持
            for index in 0..committee_for_machine_info.len() {
                if index != committee_group_index {
                    for a_committee in committee_for_machine_info[index].clone() {
                        ItemList::add_item(&mut summary.invalid_support, a_committee);
                    }
                }
            }

            if summary.against.len() > max_support_num {
                // 反对多于支持
                for a_committee in committee_for_machine_info[committee_group_index].clone() {
                    ItemList::add_item(&mut summary.invalid_support, a_committee);
                }
                return MachineConfirmStatus::Refuse(summary)
            } else if summary.against.len() == max_support_num {
                // 反对等于支持
                for a_committee in committee_for_machine_info[committee_group_index].clone() {
                    ItemList::add_item(&mut summary.invalid_support, a_committee);
                }
                summary.invalid_support = committee_for_machine_info[committee_group_index].clone();
                return MachineConfirmStatus::NoConsensus(summary)
            } else {
                // 反对小于支持
                // 记录上所有的有效支持
                summary.valid_support = committee_for_machine_info[committee_group_index].clone();
                summary.info = Some(uniq_machine_info[committee_group_index].clone());
                return MachineConfirmStatus::Confirmed(summary)
            }
        } else {
            // 如果多于两组是Max个委员会支, 则所有的支持都是无效的支持
            for index in 0..committee_for_machine_info.len() {
                for a_committee in committee_for_machine_info[index].clone() {
                    ItemList::add_item(&mut summary.invalid_support, a_committee);
                }
            }
            // Now will be Refuse or NoConsensus
            if summary.against.len() > max_support_num {
                return MachineConfirmStatus::Refuse(summary)
            } else {
                // against <= max_support 且 max_support_group > 1，且反对的不占多数
                return MachineConfirmStatus::NoConsensus(summary)
            }
        }
    }

    fn change_committee_stake(
        committee_list: Vec<T::AccountId>,
        amount: BalanceOf<T>,
        is_slash: bool,
    ) -> Result<(), ()> {
        for a_committee in committee_list {
            if is_slash {
                <T as pallet::Config>::ManageCommittee::change_total_stake(a_committee.clone(), amount, false)?;
            }

            <T as pallet::Config>::ManageCommittee::change_used_stake(a_committee, amount, false)?;
        }

        Ok(())
    }

    pub fn do_cancel_slash(slash_id: SlashId) -> DispatchResultWithPostInfo {
        ensure!(PendingSlashReview::<T>::contains_key(slash_id), Error::<T>::NotPendingReviewSlash);

        let now = <frame_system::Module<T>>::block_number();
        let mut slash_info = Self::pending_slash(slash_id);
        let slash_review_info = Self::pending_slash_review(slash_id);
        let committee_order_stake =
            <T as pallet::Config>::ManageCommittee::stake_per_order().ok_or(Error::<T>::GetStakeAmountFailed)?;

        ensure!(slash_review_info.expire_time > now, Error::<T>::ExpiredApply);

        let is_applicant_slashed_stash = match slash_info.book_result {
            OCBookResultType::OnlineRefused => &slash_info.machine_stash == &slash_review_info.applicant,
            _ => false,
        };

        // Return reserved balance when apply for review
        if is_applicant_slashed_stash {
            let _ = T::OCOperations::oc_change_staked_balance(
                slash_review_info.applicant.clone(),
                committee_order_stake,
                false,
            );
        } else {
            let _ = <T as pallet::Config>::Currency::unreserve(
                &slash_review_info.applicant,
                slash_review_info.staked_amount,
            );
            let _ = <T as Config>::ManageCommittee::change_total_stake(
                slash_review_info.applicant.clone(),
                committee_order_stake,
                false,
            );
            let _ = <T as Config>::ManageCommittee::change_used_stake(
                slash_review_info.applicant.clone(),
                committee_order_stake,
                false,
            );
        }

        let mut should_slash = slash_info.reward_committee.clone();
        let mut should_reward = slash_info.inconsistent_committee.clone();

        for a_committee in slash_info.unruly_committee.clone() {
            ItemList::add_item(&mut should_slash, a_committee);
        }
        if let OCBookResultType::OnlineRefused = slash_info.book_result {
            ItemList::add_item(&mut should_reward, slash_info.machine_stash.clone());
        }

        let _ = <T as pallet::Config>::SlashAndReward::slash_and_reward(
            should_slash,
            committee_order_stake,
            should_reward.clone(),
        );

        slash_info.slash_result = OCSlashResult::Canceled;

        // remove from unhandled report result
        let mut unhandled_slash = Self::unhandled_slash();
        ItemList::rm_item(&mut unhandled_slash, &slash_id);

        UnhandledSlash::<T>::put(unhandled_slash);
        PendingSlash::<T>::insert(slash_id, slash_info);
        PendingSlashReview::<T>::remove(slash_id);

        Ok(().into())
    }
}
