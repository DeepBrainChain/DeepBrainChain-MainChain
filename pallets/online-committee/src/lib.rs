#![cfg_attr(not(feature = "std"), no_std)]

pub mod rpc;
pub mod rpc_types;
mod slash;
mod types;

#[cfg(test)]
mod mock;
#[cfg(test)]
#[allow(non_upper_case_globals)]
mod tests;
mod utils;

use dbc_support::{
    traits::{GNOps, ManageCommittee, OCOps},
    MachineId, SlashId,
};
use frame_support::{
    ensure,
    pallet_prelude::*,
    traits::{Currency, ReservableCurrency},
};
use frame_system::{ensure_signed, pallet_prelude::*};
use generic_func::ItemList;
use online_profile::CommitteeUploadInfo;
use sp_std::{prelude::*, str, vec::Vec};

pub use pallet::*;
pub use types::*;

type BalanceOf<T> =
    <<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config + online_profile::Config + generic_func::Config + committee::Config
    {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type OCOperations: OCOps<
            AccountId = Self::AccountId,
            MachineId = MachineId,
            CommitteeUploadInfo = CommitteeUploadInfo,
            Balance = BalanceOf<Self>,
        >;
        type ManageCommittee: ManageCommittee<
            AccountId = Self::AccountId,
            Balance = BalanceOf<Self>,
        >;
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
    pub(super) type MachineCommittee<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        MachineId,
        OCMachineCommitteeList<T::AccountId, T::BlockNumber>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn machine_submited_hash)]
    pub(super) type MachineSubmitedHash<T> =
        StorageMap<_, Blake2_128Concat, MachineId, Vec<[u8; 16]>, ValueQuery>;

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

            let mut machine_submited_hash = Self::machine_submited_hash(&machine_id);
            ensure!(machine_submited_hash.binary_search(&hash).is_err(), Error::<T>::DuplicateHash);
            ItemList::add_item(&mut machine_submited_hash, hash);

            let mut machine_committee = Self::machine_committee(&machine_id);
            let mut committee_machine = Self::committee_machine(&committee);
            let mut committee_ops = Self::committee_ops(&committee, &machine_id);

            machine_committee
                .submit_hash(committee.clone())
                .map_err::<Error<T>, _>(Into::into)?;
            committee_machine.submit_hash(machine_id.clone());
            committee_ops.submit_hash(now, hash);

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
            let mut committee_ops = Self::committee_ops(&committee, &machine_id);

            ensure!(
                machine_info_detail.hash() == committee_ops.confirm_hash,
                Error::<T>::InfoNotFeatHash
            );

            committee_machine
                .submit_raw(machine_id.clone())
                .map_err::<Error<T>, _>(Into::into)?;
            machine_committee
                .submit_raw(now, committee.clone())
                .map_err::<Error<T>, _>(Into::into)?;
            committee_ops.submit_raw(now, machine_info_detail);

            CommitteeMachine::<T>::insert(&committee, committee_machine);
            MachineCommittee::<T>::insert(&machine_id, machine_committee);
            CommitteeOps::<T>::insert(&committee, &machine_id, committee_ops);

            Self::deposit_event(Event::AddConfirmRaw(committee, machine_id));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn apply_slash_review(
            origin: OriginFor<T>,
            slash_id: SlashId,
            reason: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            // 申请人
            let applicant = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let slash_info = Self::pending_slash(slash_id);
            let stake_amount = <T as Config>::ManageCommittee::stake_per_order()
                .ok_or(Error::<T>::GetStakeAmountFailed)?;

            // 确保一个惩罚只能有一个申述
            ensure!(!PendingSlashReview::<T>::contains_key(slash_id), Error::<T>::AlreadyApplied);
            ensure!(slash_info.slash_exec_time > now, Error::<T>::TimeNotAllow);

            // 判断申述人是machine_controller还是committee

            let controller_stash =
                <online_profile::Pallet<T>>::controller_stash(&applicant).unwrap_or_default();
            // 申述人是被惩罚stash账户的controller
            let is_slashed_stash = slash_info.applicant_is_stash(controller_stash.clone());
            // 申述人是被惩罚的委员会账户
            // 只允许不一致的委员会申述，未遵守规则的不允许申述
            let is_slashed_committee = slash_info.applicant_is_committee(&applicant);

            ensure!(is_slashed_stash || is_slashed_committee, Error::<T>::NotSlashed);

            let slashed = if is_slashed_stash { controller_stash } else { applicant };
            ensure!(
                <T as Config>::Currency::can_reserve(&slashed, stake_amount),
                Error::<T>::BalanceNotEnough
            );

            // 支付质押
            if is_slashed_stash {
                // 如果是stash这边申请，则质押stash的币
                T::OCOperations::oc_change_staked_balance(slashed.clone(), stake_amount, true)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;
            } else if is_slashed_committee {
                // 否则质押委员会的币
                Self::change_committee_total_stake(slashed.clone(), stake_amount, true, true)
                    .map_err(|_| Error::<T>::Overflow)?;
                Self::change_committee_used_stake(slashed.clone(), stake_amount, true)
                    .map_err(|_| Error::<T>::Overflow)?;
            }

            PendingSlashReview::<T>::insert(
                slash_id,
                OCPendingSlashReviewInfo {
                    applicant: slashed,
                    staked_amount: stake_amount,
                    apply_time: now,
                    expire_time: slash_info.slash_exec_time,
                    reason,
                },
            );

            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn cancel_slash(origin: OriginFor<T>, slash_id: SlashId) -> DispatchResultWithPostInfo {
            <T as Config>::CancelSlashOrigin::ensure_origin(origin)?;
            ensure!(
                PendingSlashReview::<T>::contains_key(slash_id),
                Error::<T>::NotPendingReviewSlash
            );

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
        Overflow,
    }
}

impl<T: Config> Pallet<T> {
    // 获取所有新加入的机器，并进行分派给委员会
    pub fn distribute_machines() {
        let live_machines = <online_profile::Pallet<T>>::live_machines();
        let now = <frame_system::Module<T>>::block_number();
        let confirm_start = now + SUBMIT_RAW_START.into();

        for machine_id in live_machines.confirmed_machine {
            // 重新分配时必须清空该状态
            if MachineCommittee::<T>::contains_key(&machine_id) {
                MachineCommittee::<T>::remove(&machine_id);
            }

            if let Some(committee_work_index) = Self::get_work_index() {
                for work_index in committee_work_index {
                    let _ = Self::book_one(machine_id.to_vec(), confirm_start, now, work_index);
                }
                // 将机器状态从ocw_confirmed_machine改为booked_machine
                T::OCOperations::oc_booked_machine(machine_id);
            };
        }
    }

    // 分派一个machineId给随机的委员会
    // 返回3个随机顺序的账户及其对应的验证顺序
    pub fn get_work_index() -> Option<Vec<VerifySequence<T::AccountId>>> {
        let mut committee = <committee::Module<T>>::available_committee()?;
        if committee.len() < 3 {
            return None
        };

        let mut verify_sequence = Vec::new();
        for i in 0..3 {
            let lucky_index =
                <generic_func::Module<T>>::random_u32(committee.len() as u32 - 1u32) as usize;
            verify_sequence.push(VerifySequence {
                who: committee[lucky_index].clone(),
                index: (i..DISTRIBUTION as usize).step_by(3).collect(),
            });
            committee.remove(lucky_index);
        }
        Some(verify_sequence)
    }

    // 一个委员会进行操作
    // - Writes: MachineCommittee, CommitteeMachine, CommitteeOps
    fn book_one(
        machine_id: MachineId,
        confirm_start: T::BlockNumber,
        now: T::BlockNumber,
        work_index: VerifySequence<T::AccountId>,
    ) -> Result<(), ()> {
        let stake_need = <T as Config>::ManageCommittee::stake_per_order().ok_or(())?;
        // Change committee usedstake will nerver fail after set proper params
        Self::change_committee_used_stake(work_index.who.clone(), stake_need, true)
            .map_err(|_| ())?;

        // 修改machine对应的委员会
        MachineCommittee::<T>::mutate(&machine_id, |machine_committee| {
            ItemList::add_item(&mut machine_committee.booked_committee, work_index.who.clone());
            machine_committee.book_time = now;
            machine_committee.confirm_start_time = confirm_start;
        });

        // 修改委员会对应的machine
        CommitteeMachine::<T>::mutate(&work_index.who, |committee_machine| {
            ItemList::add_item(&mut committee_machine.booked_machine, machine_id.clone());
        });

        // 修改委员会的操作
        CommitteeOps::<T>::mutate(&work_index.who, &machine_id, |committee_ops| {
            let start_time: Vec<_> = work_index
                .index
                .clone()
                .into_iter()
                .map(|x| now + (x as u32 * SUBMIT_RAW_START / DISTRIBUTION).into())
                .collect();

            committee_ops.staked_dbc = stake_need;
            committee_ops.verify_time = start_time;
            committee_ops.machine_status = OCMachineStatus::Booked;
        });

        Self::deposit_event(Event::MachineDistributed(machine_id.to_vec(), work_index.who));
        Ok(())
    }

    fn statistic_result() {
        let now = <frame_system::Module<T>>::block_number();
        let booked_machine = <online_profile::Pallet<T>>::live_machines().booked_machine;
        let committee_stake_per_order =
            <T as Config>::ManageCommittee::stake_per_order().unwrap_or_default();

        for machine_id in booked_machine {
            Self::summary_raw(machine_id, now, committee_stake_per_order);
        }
    }

    // 对已经提交完原始值的机器进行处理
    fn summary_raw(machine_id: MachineId, now: T::BlockNumber, stake_per_order: BalanceOf<T>) {
        let mut machine_committee = Self::machine_committee(&machine_id);

        // 如果是在提交Hash的状态，且已经到提交原始值的时间，则改变状态并返回
        if matches!(machine_committee.status, OCVerifyStatus::SubmittingHash) {
            if now >= machine_committee.book_time + SUBMIT_RAW_START.into() {
                machine_committee.status = OCVerifyStatus::SubmittingRaw;
                MachineCommittee::<T>::insert(&machine_id, machine_committee);
                return
            }
        }

        if !machine_committee.can_summary(now) {
            return
        }

        let summary_result = Self::summary_confirmation(&machine_id);
        let (inconsistent, unruly, reward) = summary_result.clone().get_committee_group();

        let mut stash_slash_info = None;

        match summary_result.clone() {
            MachineConfirmStatus::Confirmed(summary) => {
                if T::OCOperations::oc_confirm_machine(
                    summary.valid_support.clone(),
                    summary.info.unwrap(),
                )
                .is_ok()
                {
                    for a_committee in &summary.valid_support {
                        // 如果机器成功上线，则从委员会确认的机器中删除，添加到成功上线的记录中
                        CommitteeMachine::<T>::mutate(&a_committee, |committee_machine| {
                            ItemList::add_item(
                                &mut committee_machine.online_machine,
                                machine_id.clone(),
                            );
                        });
                    }
                }
            },
            MachineConfirmStatus::Refuse(_summary) => {
                // should cancel machine_stash slash when slashed committee apply review
                stash_slash_info = T::OCOperations::oc_refuse_machine(machine_id.clone());
            },
            MachineConfirmStatus::NoConsensus(_summary) => {
                let _ = Self::revert_book(machine_id.clone());
                T::OCOperations::oc_revert_booked_machine(machine_id.clone());
            },
        }

        MachineCommittee::<T>::mutate(&machine_id, |machine_committee| {
            machine_committee.after_summary(summary_result.clone())
        });

        let is_refused = summary_result.is_refused();
        if inconsistent.is_empty() && unruly.is_empty() && !is_refused {
            // 没有惩罚时则直接退还委员会的质押
            for a_committee in reward {
                let _ = Self::change_committee_used_stake(a_committee, stake_per_order, false);
            }
        } else {
            // 添加惩罚
            let slash_id = Self::get_new_slash_id();
            let (machine_stash, stash_slash_amount) = stash_slash_info.unwrap_or_default();
            PendingSlash::<T>::insert(
                slash_id,
                OCPendingSlashInfo {
                    machine_id: machine_id.clone(),
                    machine_stash,
                    stash_slash_amount,

                    inconsistent_committee: inconsistent,
                    unruly_committee: unruly,
                    reward_committee: reward,
                    committee_stake: stake_per_order,

                    slash_time: now,
                    slash_exec_time: now + TWO_DAY.into(),

                    book_result: summary_result.into_book_result(),
                    slash_result: OCSlashResult::Pending,
                },
            );

            UnhandledSlash::<T>::mutate(|unhandled_slash| {
                ItemList::add_item(unhandled_slash, slash_id);
            });
        }

        // Do cleaning
        for a_committee in machine_committee.booked_committee {
            CommitteeOps::<T>::remove(&a_committee, &machine_id);
            MachineSubmitedHash::<T>::remove(&machine_id);
            CommitteeMachine::<T>::mutate(&a_committee, |committee_machine| {
                committee_machine.online_cleanup(&machine_id)
            });
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
            CommitteeMachine::<T>::mutate(&booked_committee, |committee_machine| {
                committee_machine.revert_book(&machine_id)
            })
        }

        MachineCommittee::<T>::remove(&machine_id);
        Ok(())
    }

    // 总结机器的确认情况: 检查机器是否被确认，并检查提交的信息是否一致
    // 返回三种状态：
    // 1. 无共识：处理办法：退还委员会质押，机器重新派单。
    // 2. 支持上线: 处理办法：扣除所有反对上线，支持上线但提交无效信息的委员会的质押。
    // 3. 反对上线: 处理办法：反对的委员会平分支持的委员会的质押。扣5%矿工质押，
    // 允许矿工再次质押而上线。
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
        if machine_committee.confirmed_committee.is_empty() {
            return MachineConfirmStatus::NoConsensus(summary)
        }

        // 记录上反对上线的委员会
        for a_committee in machine_committee.confirmed_committee {
            let submit_machine_info =
                Self::committee_ops(a_committee.clone(), machine_id).machine_info;
            if !submit_machine_info.is_support {
                ItemList::add_item(&mut summary.against, a_committee);
            } else {
                match uniq_machine_info.iter().position(|r| r == &submit_machine_info) {
                    None => {
                        uniq_machine_info.push(submit_machine_info.clone());
                        committee_for_machine_info.push(vec![a_committee.clone()]);
                    },
                    Some(index) =>
                        ItemList::add_item(&mut committee_for_machine_info[index], a_committee),
                };
            }
        }

        // 统计committee_for_machine_info中有多少委员会站队最多
        let support_committee_num: Vec<usize> =
            committee_for_machine_info.iter().map(|item| item.len()).collect();
        // 最多多少个委员会达成一致意见
        let max_support = support_committee_num.clone().into_iter().max();
        if max_support.is_none() {
            // 如果没有支持者，且有反对者，则拒绝接入。
            if !summary.against.is_empty() {
                return MachineConfirmStatus::Refuse(summary)
            }
            // 反对者支持者都为0
            return MachineConfirmStatus::NoConsensus(summary)
        }

        let max_support_num = max_support.unwrap();

        // 多少个机器信息的支持等于最大的支持
        let max_support_group = support_committee_num
            .clone()
            .into_iter()
            .filter(|n| n == &max_support_num)
            .count();

        if max_support_group == 1 {
            let committee_group_index =
                support_committee_num.into_iter().position(|r| r == max_support_num).unwrap();

            // 记录所有的无效支持
            for (index, committees) in committee_for_machine_info.iter().enumerate() {
                if index != committee_group_index {
                    for a_committee in committees {
                        ItemList::add_item(&mut summary.invalid_support, a_committee.clone());
                    }
                }
            }

            if summary.against.len() > max_support_num {
                // 反对多于支持
                for a_committee in committee_for_machine_info[committee_group_index].clone() {
                    ItemList::add_item(&mut summary.invalid_support, a_committee);
                }
                MachineConfirmStatus::Refuse(summary)
            } else if summary.against.len() == max_support_num {
                // 反对等于支持
                for a_committee in committee_for_machine_info[committee_group_index].clone() {
                    ItemList::add_item(&mut summary.invalid_support, a_committee);
                }
                summary.invalid_support = committee_for_machine_info[committee_group_index].clone();
                MachineConfirmStatus::NoConsensus(summary)
            } else {
                // 反对小于支持
                // 记录上所有的有效支持
                summary.valid_support = committee_for_machine_info[committee_group_index].clone();
                summary.info = Some(uniq_machine_info[committee_group_index].clone());
                MachineConfirmStatus::Confirmed(summary)
            }
        } else {
            // 如果多于两组是Max个委员会支, 则所有的支持都是无效的支持
            for committees in &committee_for_machine_info {
                for a_committee in committees {
                    ItemList::add_item(&mut summary.invalid_support, a_committee.clone());
                }
            }
            // Now will be Refuse or NoConsensus
            if summary.against.len() > max_support_num {
                MachineConfirmStatus::Refuse(summary)
            } else {
                // against <= max_support 且 max_support_group > 1，且反对的不占多数
                MachineConfirmStatus::NoConsensus(summary)
            }
        }
    }

    pub fn do_cancel_slash(slash_id: SlashId) -> DispatchResultWithPostInfo {
        let now = <frame_system::Module<T>>::block_number();
        let mut slash_info = Self::pending_slash(slash_id);
        let slash_review_info = Self::pending_slash_review(slash_id);
        let committee_order_stake = <T as Config>::ManageCommittee::stake_per_order()
            .ok_or(Error::<T>::GetStakeAmountFailed)?;

        ensure!(slash_review_info.expire_time > now, Error::<T>::ExpiredApply);

        let is_applicant_slashed_stash =
            matches!(slash_info.book_result, OCBookResultType::OnlineRefused) &&
                slash_info.machine_stash == slash_review_info.applicant;

        // Return reserved balance when apply for review
        if is_applicant_slashed_stash {
            let _ = T::OCOperations::oc_change_staked_balance(
                slash_review_info.applicant.clone(),
                committee_order_stake,
                false,
            );
        } else {
            // 否则，申请人是被惩罚的委员会
            let _ = <T as Config>::Currency::unreserve(
                &slash_review_info.applicant,
                slash_review_info.staked_amount,
            );
            let _ = Self::change_committee_total_stake(
                slash_review_info.applicant.clone(),
                committee_order_stake,
                false,
                true,
            );
            let _ = Self::change_committee_used_stake(
                slash_review_info.applicant.clone(),
                committee_order_stake,
                false,
            );
        }

        let mut should_slash = slash_info.reward_committee.clone();
        ItemList::expand_to_order(&mut should_slash, slash_info.unruly_committee.clone());

        let mut should_reward = slash_info.inconsistent_committee.clone();
        if let OCBookResultType::OnlineRefused = slash_info.book_result {
            ItemList::add_item(&mut should_reward, slash_info.machine_stash.clone());
        }

        let _ = <T as Config>::SlashAndReward::slash_and_reward(
            should_slash.clone(),
            committee_order_stake,
            should_reward.clone(),
        );

        slash_info.slash_result = OCSlashResult::Canceled;

        // return back of reserved balance
        if is_applicant_slashed_stash {
            let _ = T::OCOperations::oc_change_staked_balance(
                slash_review_info.applicant,
                slash_info.stash_slash_amount,
                false,
            );
        }
        // 如果委员会应该被惩罚，则减少其total_stake和used_stake
        for a_committee in should_slash {
            let _ = Self::change_committee_total_stake(
                a_committee.clone(),
                committee_order_stake,
                false,
                false,
            );
            let _ = Self::change_committee_used_stake(a_committee, committee_order_stake, false);
        }
        // 如果委员会应该被奖励，则改变已使用的质押即可
        for a_committee in should_reward {
            let _ = Self::change_committee_used_stake(a_committee, committee_order_stake, false);
        }

        // remove from unhandled report result
        UnhandledSlash::<T>::mutate(|unhandled_slash| {
            ItemList::rm_item(unhandled_slash, &slash_id);
        });

        PendingSlash::<T>::insert(slash_id, slash_info);
        PendingSlashReview::<T>::remove(slash_id);

        Ok(().into())
    }
}
