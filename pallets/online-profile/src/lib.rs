#![cfg_attr(not(feature = "std"), no_std)]

mod online_reward;
mod rpc;
mod slash;
mod traits;
mod types;
mod utils;

use frame_support::{
    dispatch::DispatchResultWithPostInfo,
    pallet_prelude::*,
    traits::{Currency, EnsureOrigin, Get, OnUnbalanced, ReservableCurrency},
    weights::Weight,
};
use frame_system::pallet_prelude::*;
use generic_func::{ItemList, MachineId, SlashId};
use online_profile_machine::{DbcPrice, GNOps, ManageCommittee};
use sp_core::H256;
use sp_runtime::{
    traits::{CheckedAdd, CheckedMul, CheckedSub, Zero},
    SaturatedConversion,
};
use sp_std::{collections::btree_map::BTreeMap, convert::From, prelude::*, str, vec::Vec};

pub use pallet::*;
pub use traits::*;
pub use types::*;

type BalanceOf<T> = <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
type NegativeImbalanceOf<T> =
    <<T as pallet::Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::NegativeImbalance;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + dbc_price_ocw::Config + generic_func::Config {
        type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type Currency: ReservableCurrency<Self::AccountId>;
        type BondingDuration: Get<EraIndex>;
        type DbcPrice: DbcPrice<Balance = BalanceOf<Self>>;
        type ManageCommittee: ManageCommittee<AccountId = Self::AccountId, Balance = BalanceOf<Self>>;
        type Slash: OnUnbalanced<NegativeImbalanceOf<Self>>;
        type CancelSlashOrigin: EnsureOrigin<Self::Origin>;
        type SlashAndReward: GNOps<AccountId = Self::AccountId, Balance = BalanceOf<Self>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    #[pallet::storage]
    #[pallet::getter(fn online_stake_params)]
    pub(super) type OnlineStakeParams<T: Config> = StorageValue<_, OnlineStakeParamsInfo<BalanceOf<T>>>;

    /// A standard example for rent fee calculation(price: USD*10^6)
    #[pallet::storage]
    #[pallet::getter(fn standard_gpu_point_price)]
    pub(super) type StandardGPUPointPrice<T: Config> = StorageValue<_, StandardGpuPointPrice>;

    /// Reonline to change hardware, should stake some balance
    #[pallet::storage]
    #[pallet::getter(fn user_reonline_stake)]
    pub(super) type UserReonlineStake<T: Config> = StorageDoubleMap<
        _,
        Blake2_128Concat,
        T::AccountId,
        Blake2_128Concat,
        MachineId,
        UserReonlineStakeInfo<BalanceOf<T>, T::BlockNumber>,
        ValueQuery,
    >;

    /// If galaxy competition is begin: switch 5000 gpu
    #[pallet::storage]
    #[pallet::getter(fn galaxy_is_on)]
    pub(super) type GalaxyIsOn<T: Config> = StorageValue<_, bool, ValueQuery>;

    /// Statistics of gpu and stake
    #[pallet::storage]
    #[pallet::getter(fn sys_info)]
    pub(super) type SysInfo<T: Config> = StorageValue<_, SysInfoDetail<BalanceOf<T>>, ValueQuery>;

    /// Statistics of gpu in one position
    #[pallet::storage]
    #[pallet::getter(fn pos_gpu_info)]
    pub(super) type PosGPUInfo<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, Longitude, Blake2_128Concat, Latitude, PosInfo, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn stash_controller)]
    pub(super) type StashController<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    #[pallet::storage]
    #[pallet::getter(fn controller_stash)]
    pub(super) type ControllerStash<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, T::AccountId>;

    /// Detail info of machines
    #[pallet::storage]
    #[pallet::getter(fn machines_info)]
    pub type MachinesInfo<T: Config> =
        StorageMap<_, Blake2_128Concat, MachineId, MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>, ValueQuery>;

    /// Statistics of stash account
    #[pallet::storage]
    #[pallet::getter(fn stash_machines)]
    pub(super) type StashMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, StashMachine<BalanceOf<T>>, ValueQuery>;

    /// Server rooms in stash account
    #[pallet::storage]
    #[pallet::getter(fn stash_server_rooms)]
    pub(super) type StashServerRooms<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, Vec<H256>, ValueQuery>;

    /// All machines controlled by controller
    #[pallet::storage]
    #[pallet::getter(fn controller_machines)]
    pub(super) type ControllerMachines<T: Config> =
        StorageMap<_, Blake2_128Concat, T::AccountId, Vec<MachineId>, ValueQuery>;

    /// 系统中存储有数据的机器
    #[pallet::storage]
    #[pallet::getter(fn live_machines)]
    pub type LiveMachines<T: Config> = StorageValue<_, LiveMachine, ValueQuery>;

    /// 2880 Block/Era
    #[pallet::storage]
    #[pallet::getter(fn current_era)]
    pub type CurrentEra<T: Config> = StorageValue<_, EraIndex, ValueQuery>;

    /// 每个Era机器的得分快照
    #[pallet::storage]
    #[pallet::getter(fn eras_stash_points)]
    pub(super) type ErasStashPoints<T: Config> =
        StorageMap<_, Blake2_128Concat, EraIndex, EraStashPoints<T::AccountId>>;

    /// 每个Era机器的得分快照
    #[pallet::storage]
    #[pallet::getter(fn eras_machine_points)]
    pub(super) type ErasMachinePoints<T: Config> =
        StorageMap<_, Blake2_128Concat, EraIndex, BTreeMap<MachineId, MachineGradeStatus>>;

    #[pallet::storage]
    #[pallet::getter(fn phase_reward_info)]
    pub(super) type PhaseRewardInfo<T: Config> = StorageValue<_, PhaseRewardInfoDetail<BalanceOf<T>>>;

    #[pallet::storage]
    #[pallet::getter(fn era_reward)]
    pub(super) type EraReward<T: Config> = StorageMap<_, Blake2_128Concat, EraIndex, BalanceOf<T>, ValueQuery>;

    /// 某个Era机器获得的总奖励
    #[pallet::storage]
    #[pallet::getter(fn eras_machine_reward)]
    pub(super) type ErasMachineReward<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, EraIndex, Blake2_128Concat, MachineId, BalanceOf<T>, ValueQuery>;

    /// 某个Era机器释放的总奖励
    #[pallet::storage]
    #[pallet::getter(fn eras_machine_released_reward)]
    pub(super) type ErasMachineReleasedReward<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, EraIndex, Blake2_128Concat, MachineId, BalanceOf<T>, ValueQuery>;

    /// 某个Era Stash获得的总奖励
    #[pallet::storage]
    #[pallet::getter(fn eras_stash_reward)]
    pub(super) type ErasStashReward<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, EraIndex, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    /// 某个Era Stash解锁的总奖励
    #[pallet::storage]
    #[pallet::getter(fn eras_stash_released_reward)]
    pub(super) type ErasStashReleasedReward<T: Config> =
        StorageDoubleMap<_, Blake2_128Concat, EraIndex, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    /// 资金账户的质押总计
    #[pallet::storage]
    #[pallet::getter(fn stash_stake)]
    pub(super) type StashStake<T: Config> = StorageMap<_, Blake2_128Concat, T::AccountId, BalanceOf<T>, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn next_slash_id)]
    pub(super) type NextSlashId<T: Config> = StorageValue<_, u64, ValueQuery>;

    #[pallet::storage]
    #[pallet::getter(fn pending_slash)]
    pub(super) type PendingSlash<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        u64,
        OPPendingSlashInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        ValueQuery,
    >;

    #[pallet::storage]
    #[pallet::getter(fn pending_slash_review)]
    pub(super) type PendingSlashReview<T: Config> = StorageMap<
        _,
        Blake2_128Concat,
        SlashId,
        OPPendingSlashReviewInfo<T::AccountId, BalanceOf<T>, T::BlockNumber>,
        ValueQuery,
    >;

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn on_initialize(block_number: T::BlockNumber) -> Weight {
            if block_number.saturated_into::<u64>() % BLOCK_PER_ERA == 1 {
                // Era开始时，生成当前Era和下一个Era的快照
                // 每个Era(2880个块)执行一次
                Self::update_snap_for_new_era(block_number);
            }
            Self::check_offline_machine_duration();

            Self::do_pending_slash();
            0
        }

        fn on_finalize(block_number: T::BlockNumber) {
            let current_height = block_number.saturated_into::<u64>();
            // 在每个Era结束时执行奖励，发放到用户的Machine
            // 计算奖励，直接根据当前得分即可
            if current_height > 0 && current_height % BLOCK_PER_ERA == 0 {
                Self::distribute_reward();
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        /// When reward start to distribute
        #[pallet::weight(0)]
        pub fn set_reward_info(
            origin: OriginFor<T>,
            reward_info: PhaseRewardInfoDetail<BalanceOf<T>>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            <PhaseRewardInfo<T>>::put(reward_info);
            Ok(().into())
        }

        #[pallet::weight(0)]
        pub fn set_online_stake_params(
            origin: OriginFor<T>,
            online_stake_params_info: OnlineStakeParamsInfo<BalanceOf<T>>,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            OnlineStakeParams::<T>::put(online_stake_params_info);
            Ok(().into())
        }

        /// 设置标准GPU标准算力与租用价格
        #[pallet::weight(0)]
        pub fn set_standard_gpu_point_price(
            origin: OriginFor<T>,
            point_price: StandardGpuPointPrice,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            StandardGPUPointPrice::<T>::put(point_price);
            Ok(().into())
        }

        /// Stash account set a controller
        #[pallet::weight(10000)]
        pub fn set_controller(origin: OriginFor<T>, controller: T::AccountId) -> DispatchResultWithPostInfo {
            let stash = ensure_signed(origin)?;
            // Not allow multiple stash have same controller
            ensure!(!<ControllerStash<T>>::contains_key(&controller), Error::<T>::AlreadyController);

            StashController::<T>::insert(stash.clone(), controller.clone());
            ControllerStash::<T>::insert(controller.clone(), stash.clone());

            Self::deposit_event(Event::ControllerStashBonded(controller, stash));
            Ok(().into())
        }

        /// Stash account reset controller for one machine
        #[pallet::weight(10000)]
        pub fn stash_reset_controller(
            origin: OriginFor<T>,
            machine_id: MachineId,
            new_controller: T::AccountId,
        ) -> DispatchResultWithPostInfo {
            let stash = ensure_signed(origin)?;
            ensure!(!<ControllerStash<T>>::contains_key(&new_controller), Error::<T>::AlreadyController);

            let mut machine_info = Self::machines_info(&machine_id);
            let old_controller = machine_info.controller.clone();

            let mut old_controller_machines = Self::controller_machines(&old_controller);
            let mut new_controller_machines = Self::controller_machines(&new_controller);

            ensure!(machine_info.machine_stash == stash, Error::<T>::NotMachineStash);
            machine_info.controller = new_controller.clone();

            // Change controller_machines
            ItemList::rm_item(&mut old_controller_machines, &machine_id);
            ItemList::add_item(&mut new_controller_machines, machine_id.clone());

            ControllerMachines::<T>::insert(&old_controller, old_controller_machines);
            ControllerMachines::<T>::insert(&new_controller, new_controller_machines);
            MachinesInfo::<T>::insert(machine_id.clone(), machine_info);
            Self::deposit_event(Event::MachineControllerChanged(machine_id, old_controller, new_controller));
            Ok(().into())
        }

        /// Controller account reonline machine, allow change hardware info
        /// Committee will verify it later
        /// NOTE: User need to add machine basic info(pos & net speed), after
        /// committee verify finished, will be slashed for `OnlineReportOffline`
        #[pallet::weight(10000)]
        pub fn offline_machine_change_hardware_info(
            origin: OriginFor<T>,
            machine_id: MachineId,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let mut live_machines = Self::live_machines();
            let mut machine_info = Self::machines_info(&machine_id);

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);
            // 只允许在线状态的机器修改信息
            ensure!(machine_info.machine_status == MachineStatus::Online, Error::<T>::MachineStatusNotAllowed);

            // 重新上链需要质押一定的手续费
            let online_stake_params = Self::online_stake_params().ok_or(Error::<T>::GetReonlineStakeFailed)?;
            let stake_amount = T::DbcPrice::get_dbc_amount_by_value(online_stake_params.reonline_stake)
                .ok_or(Error::<T>::GetReonlineStakeFailed)?;

            machine_info.machine_status = MachineStatus::StakerReportOffline(now, Box::new(MachineStatus::Online));

            ItemList::rm_item(&mut live_machines.online_machine, &machine_id);
            ItemList::add_item(&mut live_machines.bonding_machine, machine_id.clone());

            Self::change_user_total_stake(machine_info.machine_stash.clone(), stake_amount, true)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;
            UserReonlineStake::<T>::insert(
                &machine_info.machine_stash,
                &machine_id,
                UserReonlineStakeInfo { stake_amount, offline_time: now },
            );
            Self::change_pos_info_by_online(&machine_info, false);
            Self::update_snap_by_online_status(machine_id.clone(), false);
            LiveMachines::<T>::put(live_machines);
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::MachineOfflineToMutHardware(machine_id, stake_amount));
            Ok(().into())
        }

        /// Controller account submit online request machine
        #[pallet::weight(10000)]
        pub fn bond_machine(
            origin: OriginFor<T>,
            machine_id: MachineId,
            msg: Vec<u8>,
            sig: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashBond)?;
            let mut live_machines = Self::live_machines();
            let mut controller_machines = Self::controller_machines(&controller);
            let mut stash_machines = Self::stash_machines(&stash);

            ensure!(!live_machines.machine_id_exist(&machine_id), Error::<T>::MachineIdExist);
            // 验证msg: len(pubkey + account) = 64 + 48
            ensure!(msg.len() == 112, Error::<T>::BadMsgLen);

            let sig_machine_id: Vec<u8> = msg[..64].to_vec();
            ensure!(machine_id == sig_machine_id, Error::<T>::SigMachineIdNotEqualBondedMachineId);

            let sig_stash_account: Vec<u8> = msg[64..].to_vec();
            let sig_stash_account =
                Self::get_account_from_str(&sig_stash_account).ok_or(Error::<T>::ConvertMachineIdToWalletFailed)?;
            ensure!(sig_stash_account == stash, Error::<T>::MachineStashNotEqualControllerStash);

            // 验证签名是否为MachineId发出
            ensure!(
                utils::verify_sig(msg.clone(), sig.clone(), machine_id.clone()).is_some(),
                Error::<T>::BadSignature
            );

            // 用户绑定机器需要质押一张显卡的DBC
            let stake_amount = Self::stake_per_gpu().ok_or(Error::<T>::CalcStakeAmountFailed)?;

            // 扣除10个Dbc作为交易手续费
            <generic_func::Module<T>>::pay_fixed_tx_fee(controller.clone()).map_err(|_| Error::<T>::PayTxFeeFailed)?;

            ItemList::add_item(&mut stash_machines.total_machine, machine_id.clone());
            ItemList::add_item(&mut controller_machines, machine_id.clone());

            // 添加到LiveMachine的bonding_machine字段
            ItemList::add_item(&mut live_machines.bonding_machine, machine_id.clone());

            // 初始化MachineInfo, 并添加到MachinesInfo
            let machine_info = MachineInfo {
                controller: controller.clone(),
                machine_stash: stash.clone(),
                bonding_height: <frame_system::Module<T>>::block_number(),
                init_stake_per_gpu: stake_amount,
                stake_amount,
                machine_status: MachineStatus::AddingCustomizeInfo,
                ..Default::default()
            };

            Self::change_user_total_stake(stash.clone(), stake_amount, true)
                .map_err(|_| Error::<T>::BalanceNotEnough)?;

            ControllerMachines::<T>::insert(&controller, controller_machines);
            StashMachines::<T>::insert(&stash, stash_machines);
            LiveMachines::<T>::put(live_machines);
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::BondMachine(controller.clone(), machine_id.clone(), stake_amount));
            Ok(().into())
        }

        /// Controller generate new server room id, record to stash account
        #[pallet::weight(10000)]
        pub fn gen_server_room(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashBond)?;

            <generic_func::Module<T>>::pay_fixed_tx_fee(controller.clone()).map_err(|_| Error::<T>::PayTxFeeFailed)?;

            let mut stash_server_rooms = Self::stash_server_rooms(&stash);
            let new_server_room = <generic_func::Module<T>>::random_server_room();
            ItemList::add_item(&mut stash_server_rooms, new_server_room);

            StashServerRooms::<T>::insert(&stash, stash_server_rooms);
            Self::deposit_event(Event::ServerRoomGenerated(controller.clone(), new_server_room));
            Ok(().into())
        }

        /// Controller add machine pos & net info
        #[pallet::weight(10000)]
        pub fn add_machine_info(
            origin: OriginFor<T>,
            machine_id: MachineId,
            customize_machine_info: StakerCustomizeInfo,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;

            ensure!(customize_machine_info.telecom_operators.len() > 0, Error::<T>::TelecomIsNull);
            // 查询机器Id是否在该账户的控制下
            let mut machine_info = Self::machines_info(&machine_id);
            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);

            let stash_server_rooms = Self::stash_server_rooms(&machine_info.machine_stash);
            ensure!(
                stash_server_rooms.binary_search(&customize_machine_info.server_room).is_ok(),
                Error::<T>::ServerRoomNotFound
            );

            match machine_info.machine_status {
                MachineStatus::AddingCustomizeInfo |
                MachineStatus::CommitteeVerifying |
                MachineStatus::CommitteeRefused(_) |
                MachineStatus::WaitingFulfill |
                MachineStatus::StakerReportOffline(_, _) => {
                    machine_info.machine_info_detail.staker_customize_info = customize_machine_info;
                },
                _ => return Err(Error::<T>::NotAllowedChangeMachineInfo.into()),
            }

            let mut live_machines = Self::live_machines();

            if live_machines.bonding_machine.binary_search(&machine_id).is_ok() {
                ItemList::rm_item(&mut live_machines.bonding_machine, &machine_id);
                ItemList::add_item(&mut live_machines.confirmed_machine, machine_id.clone());
                LiveMachines::<T>::put(live_machines);
                machine_info.machine_status = MachineStatus::DistributingOrder;
            }

            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::MachineInfoAdded(machine_id));
            Ok(().into())
        }

        /// 机器第一次上线后处于补交质押状态时，需要补交质押才能上线
        #[pallet::weight(10000)]
        pub fn fulfill_machine(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let current_era = Self::current_era();

            let mut machine_info = Self::machines_info(&machine_id);
            let mut live_machine = Self::live_machines();

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);
            ensure!(machine_info.online_height == Zero::zero(), Error::<T>::MachineStatusNotAllowed);

            // NOTE: 机器补交质押时，所需的质押 = max(当前机器需要的质押，第一次绑定上线时的质押量)
            // 每卡质押按照第一次上线时计算
            let stake_need = machine_info
                .init_stake_per_gpu
                .checked_mul(
                    &machine_info.machine_info_detail.committee_upload_info.gpu_num.saturated_into::<BalanceOf<T>>(),
                )
                .ok_or(Error::<T>::CalcStakeAmountFailed)?;

            // 当出现需要补交质押时
            if machine_info.stake_amount < stake_need {
                let extra_stake = stake_need - machine_info.stake_amount;
                Self::change_user_total_stake(machine_info.machine_stash.clone(), extra_stake, true)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;
                machine_info.stake_amount = stake_need;
            }
            machine_info.machine_status = MachineStatus::Online;

            if UserReonlineStake::<T>::contains_key(&machine_info.machine_stash, &machine_id) {
                // 根据质押，奖励给这些委员会
                let reonline_stake = Self::user_reonline_stake(&machine_info.machine_stash, &machine_id);

                // 根据下线时间，惩罚stash
                let offline_duration = now - reonline_stake.offline_time;
                // 如果下线的时候空闲超过10天，则不进行惩罚
                if reonline_stake.offline_time < machine_info.last_online_height + 28800u32.into() {
                    Self::slash_when_report_offline(
                        machine_id.clone(),
                        OPSlashReason::OnlineReportOffline(offline_duration),
                        None,
                        None,
                    );
                }
                // 退还reonline_stake
                Self::change_user_total_stake(machine_info.machine_stash.clone(), reonline_stake.stake_amount, false)
                    .map_err(|_| Error::<T>::ReduceStakeFailed)?;
                UserReonlineStake::<T>::remove(&machine_info.machine_stash, &machine_id);
            } else {
                machine_info.online_height = now;
                machine_info.reward_deadline = current_era + REWARD_DURATION;
            }

            machine_info.last_online_height = now;
            machine_info.last_machine_restake = now;

            Self::change_pos_info_by_online(&machine_info, true);
            Self::update_snap_by_online_status(machine_id.clone(), true);

            ItemList::rm_item(&mut live_machine.fulfilling_machine, &machine_id);
            ItemList::add_item(&mut live_machine.online_machine, machine_id.clone());

            LiveMachines::<T>::put(live_machine);
            MachinesInfo::<T>::insert(&machine_id, machine_info);
            Ok(().into())
        }

        /// 控制账户进行领取收益到stash账户
        #[pallet::weight(10000)]
        pub fn claim_rewards(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let stash_account = Self::controller_stash(&controller).ok_or(Error::<T>::NoStashAccount)?;

            ensure!(StashMachines::<T>::contains_key(&stash_account), Error::<T>::NotMachineController);
            let mut stash_machine = Self::stash_machines(&stash_account);
            let can_claim = stash_machine.can_claim_reward;

            stash_machine.total_claimed_reward =
                stash_machine.total_claimed_reward.checked_add(&can_claim).ok_or(Error::<T>::ClaimRewardFailed)?;
            stash_machine.can_claim_reward = Zero::zero();

            <T as pallet::Config>::Currency::deposit_into_existing(&stash_account, can_claim)
                .map_err(|_| Error::<T>::ClaimRewardFailed)?;

            StashMachines::<T>::insert(&stash_account, stash_machine);
            Self::deposit_event(Event::ClaimReward(stash_account, can_claim));
            Ok(().into())
        }

        /// 控制账户报告机器下线:Online/Rented时允许
        #[pallet::weight(10000)]
        pub fn controller_report_offline(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let machine_info = Self::machines_info(&machine_id);

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);

            // 某些状态允许下线
            match machine_info.machine_status {
                MachineStatus::Online | MachineStatus::Rented => {},
                _ => return Err(Error::<T>::MachineStatusNotAllowed.into()),
            }

            ensure!(
                now - machine_info.last_online_height > (BLOCK_PER_ERA as u32).into(),
                Error::<T>::CannotOnlineTwiceOneDay
            );

            Self::machine_offline(
                machine_id.clone(),
                MachineStatus::StakerReportOffline(now, Box::new(machine_info.machine_status)),
            );

            Self::deposit_event(Event::ControllerReportOffline(machine_id));
            Ok(().into())
        }

        /// 控制账户报告机器上线
        #[pallet::weight(10000)]
        pub fn controller_report_online(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let current_era = Self::current_era();

            let mut machine_info = Self::machines_info(&machine_id);
            let mut sys_info = Self::sys_info();
            let mut stash_machine = Self::stash_machines(&machine_info.machine_stash);
            let mut live_machine = Self::live_machines();

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);
            ensure!(
                now - machine_info.last_online_height > (BLOCK_PER_ERA as u32).into(),
                Error::<T>::CannotOnlineTwiceOneDay
            );

            let mut slash_amount = Zero::zero();
            // MachineStatus改为之前的状态
            match machine_info.machine_status.clone() {
                MachineStatus::StakerReportOffline(offline_time, status) => {
                    let offline_duration = now - offline_time;

                    machine_info.machine_status = *status;
                    let gpu_num = machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;

                    match machine_info.machine_status {
                        MachineStatus::Online | MachineStatus::Rented => {
                            // Both status will change grades by online
                            sys_info.total_gpu_num += gpu_num;

                            let old_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);

                            Self::update_snap_by_online_status(machine_id.clone(), true);
                            Self::change_pos_info_by_online(&machine_info, true);

                            let new_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);
                            stash_machine.total_calc_points =
                                stash_machine.total_calc_points + new_stash_grade - old_stash_grade;
                            sys_info.total_calc_points = sys_info.total_calc_points + new_stash_grade - old_stash_grade;

                            ItemList::add_item(&mut stash_machine.online_machine, machine_id.clone());

                            stash_machine.total_gpu_num += gpu_num;
                        },
                        _ => {},
                    }
                    match machine_info.machine_status {
                        MachineStatus::Online => {
                            // 如果在线超过10天，则不进行惩罚超过
                            if offline_time < machine_info.last_online_height + 28800u32.into() {
                                // 机器在被租用状态下线，会被惩罚
                                slash_amount = Self::slash_when_report_offline(
                                    machine_id.clone(),
                                    OPSlashReason::RentedReportOffline(offline_duration),
                                    None,
                                    None,
                                );
                            }
                        },
                        MachineStatus::Rented => {
                            Self::update_snap_by_rent_status(machine_id.clone(), true);
                            Self::change_pos_info_by_rent(&machine_info, true);

                            // 机器在被租用状态下线，会被惩罚
                            slash_amount = Self::slash_when_report_offline(
                                machine_id.clone(),
                                OPSlashReason::RentedReportOffline(offline_duration),
                                None,
                                None,
                            );
                        },
                        _ => {},
                    }
                },
                MachineStatus::ReporterReportOffline(slash_reason, _status, reporter, committee) => {
                    slash_amount = Self::slash_when_report_offline(
                        machine_id.clone(),
                        slash_reason,
                        Some(reporter),
                        Some(committee),
                    );
                },
                _ => return Err(Error::<T>::MachineStatusNotAllowed.into()),
            }

            SysInfo::<T>::put(sys_info); // Must before `change_user_total_stake`
            if slash_amount != Zero::zero() {
                Self::change_user_total_stake(machine_info.machine_stash.clone(), slash_amount, true)
                    .map_err(|_| Error::<T>::BalanceNotEnough)?;
            }

            machine_info.last_online_height = now;

            ItemList::rm_item(&mut live_machine.offline_machine, &machine_id);
            match machine_info.machine_status {
                MachineStatus::WaitingFulfill => {
                    ItemList::add_item(&mut live_machine.fulfilling_machine, machine_id.clone());
                },
                _ => {
                    ItemList::add_item(&mut live_machine.online_machine, machine_id.clone());
                },
            }

            LiveMachines::<T>::put(live_machine);
            StashMachines::<T>::insert(&machine_info.machine_stash, stash_machine);
            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::ControllerReportOnline(machine_id));
            Ok(().into())
        }

        /// 超过365天的机器可以在距离上次租用10天，且没被租用时退出
        #[pallet::weight(10000)]
        pub fn machine_exit(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let mut machine_info = Self::machines_info(&machine_id);
            let now = <frame_system::Module<T>>::block_number();
            let current_era = Self::current_era();

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);
            ensure!(MachineStatus::Online == machine_info.machine_status, Error::<T>::MachineStatusNotAllowed);
            // 确保机器：奖励结束时间 - 1年即为上线时间
            ensure!(machine_info.reward_deadline <= current_era + 365, Error::<T>::TimeNotAllowed);
            // 确保机器距离上次租用超过10天
            ensure!(now - machine_info.last_online_height >= 28800u32.into(), Error::<T>::TimeNotAllowed);

            // 下线机器，并退还奖励
            Self::change_pos_info_by_online(&machine_info, false);
            Self::update_snap_by_online_status(machine_id.clone(), false);
            ensure!(
                Self::change_user_total_stake(machine_info.machine_stash.clone(), machine_info.stake_amount, false)
                    .is_ok(),
                Error::<T>::ReduceStakeFailed
            );
            machine_info.stake_amount = Zero::zero();
            machine_info.machine_status = MachineStatus::Exit;

            MachinesInfo::<T>::insert(&machine_id, machine_info);

            Self::deposit_event(Event::MachineExit(machine_id));
            Ok(().into())
        }

        /// 满足365天可以申请重新质押，退回质押币
        /// 在系统中上线满365天之后，可以按当时机器需要的质押数量，重新入网。多余的币解绑
        /// 在重新上线之后，下次再执行本操作，需要等待365天
        #[pallet::weight(10000)]
        pub fn restake_online_machine(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();
            let mut machine_info = Self::machines_info(&machine_id);
            let old_stake = machine_info.stake_amount;

            ensure!(controller == machine_info.controller, Error::<T>::NotMachineController);
            ensure!(now - machine_info.last_machine_restake >= REBOND_FREQUENCY.into(), Error::<T>::TooFastToReStake);

            let stake_per_gpu = Self::stake_per_gpu().ok_or(Error::<T>::CalcStakeAmountFailed)?;
            let stake_need = stake_per_gpu
                .checked_mul(
                    &machine_info.machine_info_detail.committee_upload_info.gpu_num.saturated_into::<BalanceOf<T>>(),
                )
                .ok_or(Error::<T>::CalcStakeAmountFailed)?;
            ensure!(machine_info.stake_amount > stake_need, Error::<T>::NoStakeToReduce);

            let extra_stake =
                machine_info.stake_amount.checked_sub(&stake_need).ok_or(Error::<T>::ReduceStakeFailed)?;

            machine_info.stake_amount = stake_need;
            machine_info.last_machine_restake = now;
            machine_info.init_stake_per_gpu = stake_per_gpu;
            ensure!(
                Self::change_user_total_stake(machine_info.machine_stash.clone(), extra_stake, false).is_ok(),
                Error::<T>::ReduceStakeFailed
            );

            MachinesInfo::<T>::insert(&machine_id, machine_info.clone());
            Self::deposit_event(Event::MachineRestaked(machine_id, old_stake, machine_info.stake_amount));
            Ok(().into())
        }

        #[pallet::weight(10000)]
        pub fn apply_slash_review(
            origin: OriginFor<T>,
            slash_id: SlashId,
            reason: Vec<u8>,
        ) -> DispatchResultWithPostInfo {
            let controller = ensure_signed(origin)?;
            let now = <frame_system::Module<T>>::block_number();

            let slash_info = Self::pending_slash(slash_id);
            let machine_info = Self::machines_info(&slash_info.machine_id);
            let online_stake_params = Self::online_stake_params().ok_or(Error::<T>::GetReonlineStakeFailed)?;

            ensure!(machine_info.controller == controller, Error::<T>::NotMachineController);
            ensure!(slash_info.slash_exec_time > now, Error::<T>::ExpiredSlash);

            // 补交质押
            ensure!(
                Self::change_user_total_stake(
                    machine_info.machine_stash.clone(),
                    online_stake_params.slash_review_stake,
                    true,
                )
                .is_ok(),
                Error::<T>::BalanceNotEnough
            );

            PendingSlashReview::<T>::insert(
                slash_id,
                OPPendingSlashReviewInfo {
                    applicant: controller,
                    staked_amount: online_stake_params.slash_review_stake,
                    apply_time: now,
                    expire_time: slash_info.slash_exec_time,
                    reason,
                },
            );

            Self::deposit_event(Event::ApplySlashReview(slash_id));
            Ok(().into())
        }

        // FIXME: should cancel slash and slash reward
        #[pallet::weight(0)]
        pub fn cancel_slash(origin: OriginFor<T>, slash_id: u64) -> DispatchResultWithPostInfo {
            T::CancelSlashOrigin::ensure_origin(origin)?;
            ensure!(PendingSlash::<T>::contains_key(slash_id), Error::<T>::SlashIdNotExist);

            let slash_info = Self::pending_slash(slash_id);

            Self::change_user_total_stake(slash_info.slash_who.clone(), slash_info.slash_amount, false)
                .map_err(|_| Error::<T>::ReduceStakeFailed)?;
            PendingSlash::<T>::remove(slash_id);

            Self::deposit_event(Event::SlashCanceled(slash_id, slash_info.slash_who, slash_info.slash_amount));
            Ok(().into())
        }
    }

    #[pallet::event]
    #[pallet::metadata(T::AccountId = "AccountId", BalanceOf<T> = "Balance")]
    #[pallet::generate_deposit(pub(super) fn deposit_event)]
    pub enum Event<T: Config> {
        BondMachine(T::AccountId, MachineId, BalanceOf<T>),
        Slash(T::AccountId, BalanceOf<T>, OPSlashReason<T::BlockNumber>),
        ControllerStashBonded(T::AccountId, T::AccountId),
        MachineControllerChanged(MachineId, T::AccountId, T::AccountId),
        MachineOfflineToMutHardware(MachineId, BalanceOf<T>),
        StakeAdded(T::AccountId, BalanceOf<T>),
        StakeReduced(T::AccountId, BalanceOf<T>),
        ServerRoomGenerated(T::AccountId, H256),
        MachineInfoAdded(MachineId),
        ClaimReward(T::AccountId, BalanceOf<T>),
        ControllerReportOffline(MachineId),
        ControllerReportOnline(MachineId),
        SlashCanceled(u64, T::AccountId, BalanceOf<T>),
        // machine_id, old_stake, new_stake
        MachineRestaked(MachineId, BalanceOf<T>, BalanceOf<T>),
        MachineExit(MachineId),
        // Slash_who, reward_who, reward_amount
        SlashAndReward(T::AccountId, T::AccountId, BalanceOf<T>, OPSlashReason<T::BlockNumber>),
        ApplySlashReview(SlashId),
    }

    #[pallet::error]
    pub enum Error<T> {
        BadSignature,
        MachineIdExist,
        BalanceNotEnough,
        NotMachineController,
        PayTxFeeFailed,
        RewardPhaseOutOfRange,
        ClaimRewardFailed,
        ConvertMachineIdToWalletFailed,
        NoStashBond,
        AlreadyController,
        NoStashAccount,
        BadMsgLen,
        NotAllowedChangeMachineInfo,
        MachineStashNotEqualControllerStash,
        CalcStakeAmountFailed,
        NotRefusedMachine,
        SigMachineIdNotEqualBondedMachineId,
        TelecomIsNull,
        MachineStatusNotAllowed,
        CannotOnlineTwiceOneDay,
        ServerRoomNotFound,
        NotMachineStash,
        TooFastToReStake,
        NoStakeToReduce,
        ReduceStakeFailed,
        GetReonlineStakeFailed,
        SlashIdNotExist,
        TimeNotAllowed,
        ExpiredSlash,
    }
}

impl<T: Config> Pallet<T> {
    /// 下架机器
    fn machine_offline(machine_id: MachineId, machine_status: MachineStatus<T::BlockNumber, T::AccountId>) {
        let mut machine_info = Self::machines_info(&machine_id);
        let mut live_machine = Self::live_machines();

        if let MachineStatus::Rented = machine_info.machine_status {
            Self::update_snap_by_rent_status(machine_id.clone(), false);
            Self::change_pos_info_by_rent(&machine_info, false);
        }

        // When offline, pos_info will be removed
        Self::change_pos_info_by_online(&machine_info, false);
        Self::update_snap_by_online_status(machine_id.clone(), false);

        ItemList::rm_item(&mut live_machine.online_machine, &machine_id);
        ItemList::add_item(&mut live_machine.offline_machine, machine_id.clone());

        // After re-online, machine status is same as former
        machine_info.machine_status = machine_status;

        LiveMachines::<T>::put(live_machine);
        MachinesInfo::<T>::insert(&machine_id, machine_info);
    }

    /// GPU online/offline
    // - Writes: PosGPUInfo
    // NOTE: pos_gpu_info only record actual machine grades(reward grade not included)
    fn change_pos_info_by_online(
        machine_info: &MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        is_online: bool,
    ) {
        let longitude = &machine_info.machine_info_detail.staker_customize_info.longitude;
        let latitude = &machine_info.machine_info_detail.staker_customize_info.latitude;
        let gpu_num = machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;
        let calc_point = machine_info.machine_info_detail.committee_upload_info.calc_point;

        let mut pos_gpu_info = Self::pos_gpu_info(longitude, latitude);

        if is_online {
            pos_gpu_info.online_gpu += gpu_num;
            pos_gpu_info.online_gpu_calc_points += calc_point;
        } else {
            pos_gpu_info.online_gpu = pos_gpu_info.online_gpu.checked_sub(gpu_num).unwrap_or_default();
            pos_gpu_info.offline_gpu += gpu_num;
            pos_gpu_info.online_gpu_calc_points =
                pos_gpu_info.online_gpu_calc_points.checked_sub(calc_point).unwrap_or_default();
        }

        PosGPUInfo::<T>::insert(longitude, latitude, pos_gpu_info);
    }

    /// GPU rented/surrender
    // - Writes: PosGPUInfo
    fn change_pos_info_by_rent(
        machine_info: &MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        is_rented: bool,
    ) {
        let longitude = &machine_info.machine_info_detail.staker_customize_info.longitude;
        let latitude = &machine_info.machine_info_detail.staker_customize_info.latitude;
        let gpu_num = machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;

        let mut pos_gpu_info = Self::pos_gpu_info(longitude.clone(), latitude.clone());
        if is_rented {
            pos_gpu_info.rented_gpu += gpu_num;
        } else {
            pos_gpu_info.rented_gpu = pos_gpu_info.rented_gpu.checked_sub(gpu_num).unwrap_or_default();
        }

        PosGPUInfo::<T>::insert(longitude, latitude, pos_gpu_info);
    }

    fn change_user_total_stake(who: T::AccountId, amount: BalanceOf<T>, is_add: bool) -> Result<(), ()> {
        let mut stash_stake = Self::stash_stake(&who);
        let mut sys_info = Self::sys_info();

        if is_add {
            sys_info.total_stake = sys_info.total_stake.checked_add(&amount).ok_or(())?;
            stash_stake = stash_stake.checked_add(&amount).ok_or(())?;

            ensure!(<T as Config>::Currency::can_reserve(&who, amount), ());
            <T as pallet::Config>::Currency::reserve(&who, amount).map_err(|_| ())?;
        } else {
            stash_stake = stash_stake.checked_sub(&amount).ok_or(())?;
            sys_info.total_stake = sys_info.total_stake.checked_sub(&amount).ok_or(())?;
            <T as pallet::Config>::Currency::unreserve(&who, amount);
        }

        StashStake::<T>::insert(&who, stash_stake);
        SysInfo::<T>::put(sys_info);

        if is_add {
            Self::deposit_event(Event::StakeAdded(who, amount));
        } else {
            Self::deposit_event(Event::StakeReduced(who, amount));
        }
        Ok(())
    }

    // 获取下一Era stash grade即为当前Era stash grade
    fn get_stash_grades(era_index: EraIndex, stash: &T::AccountId) -> u64 {
        let next_era_stash_snapshot = Self::eras_stash_points(era_index).unwrap_or_default();

        if let Some(stash_snapshot) = next_era_stash_snapshot.staker_statistic.get(stash) {
            return stash_snapshot.total_grades().unwrap_or_default()
        }
        0
    }

    // When Online:
    // - Writes:(currentEra) ErasStashPoints, ErasMachinePoints,
    //   SysInfo, StashMachines
    // When Offline:
    // - Writes: (currentEra) ErasStashPoints, ErasMachinePoints, (nextEra) ErasStashPoints, ErasMachinePoints
    //   SysInfo, StashMachines
    fn update_snap_by_online_status(machine_id: MachineId, is_online: bool) {
        let machine_info = Self::machines_info(&machine_id);
        let machine_base_info = machine_info.machine_info_detail.committee_upload_info.clone();
        let current_era = Self::current_era();

        let mut current_era_stash_snapshot = Self::eras_stash_points(current_era).unwrap();
        let mut next_era_stash_snapshot = Self::eras_stash_points(current_era + 1).unwrap();
        let mut current_era_machine_snapshot = Self::eras_machine_points(current_era).unwrap();
        let mut next_era_machine_snapshot = Self::eras_machine_points(current_era + 1).unwrap();

        let mut stash_machine = Self::stash_machines(&machine_info.machine_stash);
        let mut sys_info = Self::sys_info();

        let old_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);

        next_era_stash_snapshot.change_machine_online_status(
            machine_info.machine_stash.clone(),
            machine_info.machine_info_detail.committee_upload_info.gpu_num as u64,
            machine_info.machine_info_detail.committee_upload_info.calc_point,
            is_online,
        );

        if is_online {
            next_era_machine_snapshot.insert(
                machine_id.clone(),
                MachineGradeStatus {
                    basic_grade: machine_info.machine_info_detail.committee_upload_info.calc_point,
                    is_rented: false,
                },
            );

            ItemList::add_item(&mut stash_machine.online_machine, machine_id.clone());
            stash_machine.total_gpu_num += machine_base_info.gpu_num as u64;
            sys_info.total_gpu_num += machine_base_info.gpu_num as u64;
        } else {
            // NOTE: 24小时内，不能下线后再次下线。因为下线会清空当日得分记录，
            // 一天内再次下线会造成再次清空
            current_era_stash_snapshot.change_machine_online_status(
                machine_info.machine_stash.clone(),
                machine_info.machine_info_detail.committee_upload_info.gpu_num as u64,
                machine_info.machine_info_detail.committee_upload_info.calc_point,
                is_online,
            );
            current_era_machine_snapshot.remove(&machine_id);
            next_era_machine_snapshot.remove(&machine_id);

            ItemList::rm_item(&mut stash_machine.online_machine, &machine_id);
            stash_machine.total_gpu_num -= machine_base_info.gpu_num as u64;
            sys_info.total_gpu_num -= machine_base_info.gpu_num as u64;
        }

        // 机器上线或者下线都会影响下一era得分，而只有下线才影响当前era得分
        ErasStashPoints::<T>::insert(current_era + 1, next_era_stash_snapshot);
        ErasMachinePoints::<T>::insert(current_era + 1, next_era_machine_snapshot);
        if !is_online {
            ErasStashPoints::<T>::insert(current_era, current_era_stash_snapshot);
            ErasMachinePoints::<T>::insert(current_era, current_era_machine_snapshot);
        }

        let new_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);
        stash_machine.total_calc_points = stash_machine.total_calc_points + new_stash_grade - old_stash_grade;
        sys_info.total_calc_points = sys_info.total_calc_points + new_stash_grade - old_stash_grade;

        // NOTE: 5000张卡开启银河竞赛
        if !Self::galaxy_is_on() && sys_info.total_gpu_num > 5000 {
            GalaxyIsOn::<T>::put(true);
        }

        if is_online && stash_machine.online_machine.len() == 1 {
            sys_info.total_staker += 1;
        }
        if !is_online && stash_machine.online_machine.len() == 0 {
            sys_info.total_staker -= 1;
        }

        SysInfo::<T>::put(sys_info);
        StashMachines::<T>::insert(&machine_info.machine_stash, stash_machine);
    }

    // - Writes:
    // ErasStashPoints, ErasMachinePoints, SysInfo, StashMachines
    // TODO: refa: only change one_era is enough
    fn update_snap_by_rent_status(machine_id: MachineId, is_rented: bool) {
        let machine_info = Self::machines_info(&machine_id);
        let current_era = Self::current_era();

        let mut current_era_stash_snap = Self::eras_stash_points(current_era).unwrap_or_default();
        let mut next_era_stash_snap = Self::eras_stash_points(current_era + 1).unwrap_or_default();
        let mut current_era_machine_snap = Self::eras_machine_points(current_era).unwrap_or_default();
        let mut next_era_machine_snap = Self::eras_machine_points(current_era + 1).unwrap_or_default();

        let mut stash_machine = Self::stash_machines(&machine_info.machine_stash);
        let mut sys_info = Self::sys_info();

        let old_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);

        next_era_stash_snap.change_machine_rent_status(
            machine_info.machine_stash.clone(),
            machine_info.machine_info_detail.committee_upload_info.calc_point,
            is_rented,
        );
        next_era_machine_snap.insert(
            machine_id.clone(),
            MachineGradeStatus {
                basic_grade: machine_info.machine_info_detail.committee_upload_info.calc_point,
                is_rented,
            },
        );

        if !is_rented {
            current_era_stash_snap.change_machine_rent_status(
                machine_info.machine_stash.clone(),
                machine_info.machine_info_detail.committee_upload_info.calc_point,
                is_rented,
            );

            current_era_machine_snap.insert(
                machine_id.clone(),
                MachineGradeStatus {
                    basic_grade: machine_info.machine_info_detail.committee_upload_info.calc_point,
                    is_rented,
                },
            );
        }

        // 被租用或者退租都影响下一Era记录，而退租直接影响当前得分
        ErasStashPoints::<T>::insert(current_era + 1, next_era_stash_snap);
        ErasMachinePoints::<T>::insert(current_era + 1, next_era_machine_snap);
        if !is_rented {
            ErasStashPoints::<T>::insert(current_era, current_era_stash_snap);
            ErasMachinePoints::<T>::insert(current_era, current_era_machine_snap);
            sys_info.total_rented_gpu -= machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;
            stash_machine.total_rented_gpu -= machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;
        } else {
            sys_info.total_rented_gpu += machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;
            stash_machine.total_rented_gpu += machine_info.machine_info_detail.committee_upload_info.gpu_num as u64;
        }

        let new_stash_grade = Self::get_stash_grades(current_era + 1, &machine_info.machine_stash);
        stash_machine.total_calc_points = stash_machine.total_calc_points + new_stash_grade - old_stash_grade;
        sys_info.total_calc_points = sys_info.total_calc_points + new_stash_grade - old_stash_grade;

        SysInfo::<T>::put(sys_info);
        StashMachines::<T>::insert(&machine_info.machine_stash, stash_machine);
    }
}
