#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]
#![warn(unused_crate_dependencies)]

pub mod rpc;
pub mod rpc_types;
mod slash;
mod types;

mod migrations;
#[cfg(test)]
mod mock;
#[cfg(test)]
#[allow(non_upper_case_globals)]
mod tests;
mod utils;

use dbc_support::{
    machine_type::CommitteeUploadInfo,
    traits::{GNOps, ManageCommittee, OCOps},
    utils::OnlineCommitteeSummary,
    verify_committee_slash::{OCPendingSlashInfo, OCSlashResult},
    verify_online::{
        OCBookResultType, OCCommitteeMachineList, OCCommitteeOps, OCMachineCommitteeList,
        OCMachineStatus, OCVerifyStatus, Summary, VerifyResult, VerifySequence, SUBMIT_RAW_START,
    },
    ItemList, MachineId, SlashId, TWO_DAY,
};
use frame_support::{
    ensure,
    pallet_prelude::*,
    traits::{Currency, ReservableCurrency},
};
use frame_system::{ensure_signed, pallet_prelude::*};
use sp_runtime::traits::Zero;
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
        type RuntimeEvent: From<Event<Self>> + IsType<<Self as frame_system::Config>::RuntimeEvent>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type OCOps: OCOps<
            AccountId = Self::AccountId,
            MachineId = MachineId,
            CommitteeUploadInfo = CommitteeUploadInfo,
            Balance = BalanceOf<Self>,
        >;
        type ManageCommittee: ManageCommittee<
            AccountId = Self::AccountId,
            Balance = BalanceOf<Self>,
        >;
        type CancelSlashOrigin: EnsureOrigin<Self::RuntimeOrigin>;
        type SlashAndReward: GNOps<AccountId = Self::AccountId, Balance = BalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::without_storage_info]
    pub struct Pallet<T>(_);

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(_n: BlockNumberFor<T>) -> frame_support::weights::Weight {
            Self::check_and_exec_pending_review();
            Self::check_and_exec_pending_slash();
            Weight::zero()
        }

        fn on_finalize(_block_number: T::BlockNumber) {
            Self::statistic_result();
            Self::distribute_machines();
        }

        // fn on_runtime_upgrade() -> frame_support::weights::Weight {
        //     frame_support::log::info!("🔍 OnlineCommittee Storage Migration start");
        //     migrations::migrate::<T>();
        //     frame_support::log::info!("🚀 OnlineCommittee Storage Migration end");
        //     Weight::zero()
        // }
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
    >;

    #[pallet::storage]
    #[pallet::getter(fn pending_slash_review)]
    pub(super) type PendingSlashReview<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        SlashId,
        OCPendingSlashReviewInfo<T::AccountId, BalanceOf<T>, T::BlockNumber>,
    >;

    #[pallet::storage]
    #[pallet::getter(fn unhandled_slash)]
    pub(super) type UnhandledSlash<T: Config> = StorageValue<_, Vec<SlashId>, ValueQuery>;

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        #[pallet::call_index(0)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn submit_confirm_hash(
            origin: OriginFor<T>,
            machine_id: MachineId,
            hash: [u8; 16],
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Pallet<T>>::block_number();

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

        #[pallet::call_index(1)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn submit_confirm_raw(
            origin: OriginFor<T>,
            machine_info_detail: CommitteeUploadInfo,
        ) -> DispatchResultWithPostInfo {
            let committee = ensure_signed(origin)?;
            let now = <frame_system::Pallet<T>>::block_number();
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

        #[pallet::call_index(2)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
        pub fn apply_slash_review(
            origin: OriginFor<T>,
            slash_id: SlashId,
            reason: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            // 申请人
            let applicant = ensure_signed(origin)?;
            let now = <frame_system::Pallet<T>>::block_number();
            let slash_info = Self::pending_slash(slash_id).ok_or(Error::<T>::Unknown)?;
            let stake_amount = <T as Config>::ManageCommittee::stake_per_order()
                .ok_or(Error::<T>::GetStakeAmountFailed)?;

            // 确保一个惩罚只能有一个申述
            ensure!(!PendingSlashReview::<T>::contains_key(slash_id), Error::<T>::AlreadyApplied);
            ensure!(slash_info.slash_exec_time > now, Error::<T>::TimeNotAllow);

            // 判断申述人是machine_controller还是committee

            let controller_stash = <online_profile::Pallet<T>>::controller_stash(&applicant)
                .ok_or(Error::<T>::Unknown)?;
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
                T::OCOps::change_staked_balance(slashed.clone(), stake_amount, true)
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

        #[pallet::call_index(3)]
        #[pallet::weight(frame_support::weights::Weight::from_parts(10000, 0))]
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
    // #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
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
        Unknown,
    }
}

impl<T: Config> Pallet<T> {
    // 获取所有新加入的机器，并进行分派给委员会
    pub fn distribute_machines() {
        let live_machines = <online_profile::Pallet<T>>::live_machines();
        let now = <frame_system::Pallet<T>>::block_number();
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
                let _ = T::OCOps::booked_machine(machine_id);
            };
        }
    }

    // 分派一个machineId给随机的委员会
    // 返回3个随机顺序的账户及其对应的验证顺序
    pub fn get_work_index() -> Option<Vec<VerifySequence<T::AccountId>>> {
        let mut committee = <committee::Pallet<T>>::available_committee()?;
        if committee.len() < 3 {
            return None;
        };

        let mut verify_sequence = Vec::new();
        for i in 0..3 {
            let lucky_index =
                <generic_func::Pallet<T>>::random_u32(committee.len() as u32) as usize;
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
            // let machine_committee = machine_committee.as_mut().ok_or(())?;
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
        let now = <frame_system::Pallet<T>>::block_number();
        let booked_machine = <online_profile::Pallet<T>>::live_machines().booked_machine;
        let committee_stake_per_order =
            <T as Config>::ManageCommittee::stake_per_order().unwrap_or_default();

        for machine_id in booked_machine {
            let _ = Self::summary_raw(machine_id, now, committee_stake_per_order);
        }
    }

    // 对已经提交完原始值的机器进行处理
    fn summary_raw(
        machine_id: MachineId,
        now: T::BlockNumber,
        stake_per_order: BalanceOf<T>,
    ) -> Result<(), ()> {
        let mut machine_committee = Self::machine_committee(&machine_id);

        // 如果是在提交Hash的状态，且已经到提交原始值的时间，则改变状态并返回
        if machine_committee.can_submit_raw(now) {
            machine_committee.status = OCVerifyStatus::SubmittingRaw;
            MachineCommittee::<T>::insert(&machine_id, machine_committee);
            return Ok(());
        }
        if !machine_committee.can_summary(now) {
            return Ok(());
        }

        let mut submit_info = vec![];
        machine_committee.confirmed_committee.iter().for_each(|a_committee| {
            submit_info.push(Self::committee_ops(a_committee, &machine_id).machine_info);
        });
        let summary = Self::summary_confirmation(machine_committee.clone(), submit_info);

        let stash_slash = if matches!(summary.verify_result, VerifyResult::Refused) {
            T::OCOps::refuse_machine(summary.valid_vote.clone(), machine_id.clone())
        } else {
            None
        };

        match summary.verify_result.clone() {
            VerifyResult::Confirmed => {
                T::OCOps::confirm_machine(
                    summary.valid_vote.clone(),
                    summary.info.clone().unwrap(),
                )?;
                summary.valid_vote.iter().for_each(|a_committee| {
                    // TODO: 如果机器成功上线，则从委员会确认的机器中删除，添加到成功上线的记录中
                    CommitteeMachine::<T>::mutate(&a_committee, |record| {
                        ItemList::add_item(&mut record.online_machine, machine_id.clone());
                    });
                });
            },
            VerifyResult::Refused => {},
            VerifyResult::NoConsensus => {
                let _ = Self::revert_book(machine_id.clone());
                T::OCOps::revert_booked_machine(machine_id.clone())?;

                for a_committee in summary.invalid_vote.clone() {
                    let _ = Self::change_committee_used_stake(a_committee, stake_per_order, false);
                }
            },
        }

        // NOTE: 添加惩罚
        if stash_slash.is_some() || summary.should_slash_committee() {
            let (machine_stash, stash_slash_amount) = if let Some(tmp) = stash_slash {
                (Some(tmp.0), tmp.1)
            } else {
                (None, Zero::zero())
            };

            // let (machine_stash, stash_slash_amount) = stash_slash;
            Self::add_summary_slash(
                machine_id.clone(),
                machine_stash,
                stash_slash_amount,
                summary.clone(),
                stake_per_order,
                now,
            );
        } else {
            // NOTE: 没有任何惩罚时退还正确质押委员会的质押
            // 否则，还需要质押到两天之后惩罚执行时，才退还！
            for a_committee in summary.valid_vote.clone() {
                let _ = Self::change_committee_used_stake(a_committee, stake_per_order, false);
            }
        }

        MachineCommittee::<T>::mutate(&machine_id, |machine_committee| {
            // let machine_committee = machine_committee.as_mut().ok_or(())?;
            machine_committee.after_summary(summary.clone());
        });

        // Do cleaning
        for a_committee in machine_committee.booked_committee {
            CommitteeOps::<T>::remove(&a_committee, &machine_id);
            MachineSubmitedHash::<T>::remove(&machine_id);
            CommitteeMachine::<T>::mutate(&a_committee, |committee_machine| {
                committee_machine.online_cleanup(&machine_id)
            });
        }
        Ok(())
    }

    fn add_summary_slash(
        machine_id: MachineId,
        machine_stash: Option<T::AccountId>,
        slash_amount: BalanceOf<T>,
        summary: Summary<T::AccountId>,
        stake_per_order: BalanceOf<T>,
        now: T::BlockNumber,
    ) {
        let slash_id = Self::get_new_slash_id();
        PendingSlash::<T>::insert(
            slash_id,
            OCPendingSlashInfo {
                machine_id: machine_id.clone(),
                machine_stash,
                stash_slash_amount: slash_amount,

                inconsistent_committee: summary.invalid_vote.clone(),
                unruly_committee: summary.unruly.clone(),
                reward_committee: summary.valid_vote.clone(),
                committee_stake: stake_per_order,

                slash_time: now,
                slash_exec_time: now + TWO_DAY.into(),

                book_result: summary.into_book_result(),
                slash_result: OCSlashResult::Pending,
            },
        );
        UnhandledSlash::<T>::mutate(|unhandled_slash| {
            ItemList::add_item(unhandled_slash, slash_id);
        });
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

    pub fn do_cancel_slash(slash_id: SlashId) -> DispatchResultWithPostInfo {
        let now = <frame_system::Pallet<T>>::block_number();
        let mut slash_info = Self::pending_slash(slash_id).ok_or(Error::<T>::Unknown)?;
        let slash_review_info = Self::pending_slash_review(slash_id).ok_or(Error::<T>::Unknown)?;
        let committee_order_stake = <T as Config>::ManageCommittee::stake_per_order()
            .ok_or(Error::<T>::GetStakeAmountFailed)?;

        ensure!(slash_review_info.expire_time > now, Error::<T>::ExpiredApply);

        let is_applicant_slashed_stash =
            matches!(slash_info.book_result, OCBookResultType::OnlineRefused) &&
                slash_info.machine_stash == Some(slash_review_info.applicant.clone());

        // Return reserved balance when apply for review
        if is_applicant_slashed_stash {
            let _ = T::OCOps::change_staked_balance(
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
        if matches!(slash_info.book_result, OCBookResultType::OnlineRefused) {
            if let Some(machine_stash) = slash_info.machine_stash.clone() {
                ItemList::add_item(&mut should_reward, machine_stash);
            }
        }

        let _ = <T as Config>::SlashAndReward::slash_and_reward(
            should_slash.clone(),
            committee_order_stake,
            should_reward.clone(),
        );

        slash_info.slash_result = OCSlashResult::Canceled;

        // return back of reserved balance
        if is_applicant_slashed_stash {
            let _ = T::OCOps::change_staked_balance(
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

impl<T: Config> OnlineCommitteeSummary for Pallet<T> {
    type AccountId = T::AccountId;
    type BlockNumber = T::BlockNumber;
}
