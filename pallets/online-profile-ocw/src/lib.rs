#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{debug, dispatch::DispatchResultWithPostInfo, pallet_prelude::*};
use frame_system::{
    offchain::{CreateSignedTransaction, SubmitTransaction},
    pallet_prelude::*,
};
use online_profile::types::*;
use online_profile_machine::{LCOps, OPOps};
use sp_runtime::{offchain, offchain::http, traits::SaturatedConversion};
use sp_std::{convert::TryInto, prelude::*, str};

pub mod machine_info;

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub type MachineId = Vec<u8>;
pub const UNSIGNED_TXS_PRIORITY: u64 = 100;

#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config:
        frame_system::Config
        + CreateSignedTransaction<Call<Self>>
        + online_profile::Config
        + random_num::Config
    {
        // type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type OnlineProfile: LCOps<MachineId = MachineId>
            + OPOps<
                AccountId = Self::AccountId,
                BondingPair = BondingPair<Self::AccountId>,
                BookingItem = BookingItem<Self::BlockNumber>,
                MachineId = MachineId,
                ConfirmedMachine = ConfirmedMachine<Self::AccountId, Self::BlockNumber>,
            >;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    // 添加 machineInfoURL, 并进行随机选择一些节点
    // eg: pub MachineInfoUrl get(fn machine_info_url) config(): MachineId = "http://116.85.24.172:41107/api/v1/mining_nodes/".as_bytes().to_vec();
    #[pallet::storage]
    #[pallet::getter(fn machine_info_url)]
    pub(super) type MachineInfoURL<T> = StorageValue<_, Vec<MachineId>, ValueQuery>;

    // /// OCW query from _ nodes
    // pub MachineInfoRandURLNum get(fn machine_info_rand_url_num) config(): u32 = 3;
    #[pallet::type_value]
    pub fn MachineInfoRandURLNumDefault<T: Config>() -> u32 {
        3
    }

    #[pallet::storage]
    #[pallet::getter(fn machine_info_rand_url_num)]
    pub(super) type MachineInfoRandURLNum<T: Config> =
        StorageValue<_, u32, ValueQuery, MachineInfoRandURLNumDefault<T>>;

    #[pallet::type_value]
    pub fn RequestLimitDefault<T: Config>() -> u64 {
        3
    }

    #[pallet::storage]
    #[pallet::getter(fn request_limit)]
    pub(super) type RequestLimit<T> = StorageValue<_, u64, ValueQuery, RequestLimitDefault<T>>;

    /// random url for machine info
    #[pallet::storage]
    #[pallet::getter(fn machine_info_rand_url)]
    pub(super) type MachineInfoRandURL<T> = StorageValue<_, Vec<MachineId>, ValueQuery>;

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;
        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            let valid_tx = |provide| {
                ValidTransaction::with_tag_prefix("online-profile")
                    .priority(UNSIGNED_TXS_PRIORITY)
                    .and_provides([&provide])
                    .longevity(3)
                    .propagate(true)
                    .build()
            };

            match call {
                Call::ocw_submit_machine_info() => valid_tx(b"ocw_submit_machine_info".to_vec()),
                _ => InvalidTransaction::Call.into(),
            }
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(block_number: T::BlockNumber) {
            debug::info!("Entering off-chain worker, at height: {:?}", block_number);

            let result = Self::call_ocw_machine_info();
            if let Err(e) = result {
                debug::error!("offchain_worker error: {:?}", e);
            }
        }

        fn on_finalize(block_number: T::BlockNumber) {
            if block_number.saturated_into::<u64>() / 10 == 0 {
                Self::update_machine_info_url()
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // root用户添加机器信息API
        #[pallet::weight(0)]
        fn add_machine_info_url(
            origin: OriginFor<T>,
            new_url: MachineId,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut machine_info_url = MachineInfoURL::<T>::get();

            machine_info_url.push(new_url.clone());
            MachineInfoURL::<T>::put(machine_info_url);

            Ok(().into())
        }

        // root用户删除机器信息API
        /// Rm URL for OCW query machine info
        #[pallet::weight(0)]
        fn rm_url_by_index(origin: OriginFor<T>, index: u32) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut machine_info_url = MachineInfoURL::<T>::get();

            ensure!(
                index > machine_info_url.len() as u32,
                Error::<T>::IndexOutOfRange
            );
            machine_info_url.remove(index as usize);
            MachineInfoURL::<T>::put(machine_info_url);

            Ok(().into())
        }

        // root用户设置随机选择多少API进行验证机器信息
        #[pallet::weight(0)]
        fn set_machineinfo_rand_url_num(
            origin: OriginFor<T>,
            num: u32,
        ) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            MachineInfoRandURLNum::<T>::put(num);
            Ok(().into())
        }

        // ocw 实现获取machine info并发送unsigned tx以修改到存储
        // UserBondedMachine增加who-machine_id pair;
        // BondedMachineId 增加 machine_id => ()
        // BondingQueueMachineId 减少 machine_id
        #[pallet::weight(0)]
        fn ocw_submit_machine_info(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;

            let bonding_queue_id = T::OnlineProfile::bonding_queue_id();
            // let booking_queue_id = Self::booking_queue_id();

            let request_limit = RequestLimit::<T>::get();

            let machine_info_url = MachineInfoRandURL::<T>::get();
            ensure!(machine_info_url.len() != 0, Error::<T>::MachineURLEmpty);

            for machine_id in bonding_queue_id.iter() {
                let bonding_pair = T::OnlineProfile::get_bonding_pair(machine_id.to_vec());
                // let bonding_pair = T::BondingQueue::<T>::get(&machine_id);
                let mut request_count = bonding_pair.request_count;

                let mut machine_grade: Vec<MachineGradeDetail> = vec![];
                let mut appraisal_price: Vec<u64> = vec![];

                for url in machine_info_url.iter() {
                    let machine_info = Self::fetch_machine_info(&url, &bonding_pair.machine_id);
                    if let Err(e) = machine_info {
                        // TODO: handle 404的情况(machine_id not found)
                        request_count += 1; // 可以将该逻辑改到Err(e)为404时触发
                        if request_count >= request_limit {
                            // TODO: 增加log提示
                            // T::BondingQueue::<T>::remove(machine_id);
                            T::OnlineProfile::rm_bonding_id(machine_id.to_vec());
                            break;
                        }
                        debug::error!("Offchain worker error: {:?}", e);
                        continue;
                    }
                    let machine_info = machine_info.unwrap();

                    let machine_wallet = &machine_info.data.wallet[1].0;

                    debug::info!("machine info is: {:?}", &machine_wallet);

                    // 如果不一致，则直接进行下一个machine_id的查询
                    if Self::wallet_match_account(bonding_pair.account_id.clone(), machine_wallet) {
                        // TODO: 增加log提示
                        // T::BondingQueue::<T>::remove(machine_id);
                        T::OnlineProfile::rm_bonding_id(machine_id.to_vec());
                        break;
                    }

                    let grades = &machine_info.data.grades;

                    machine_grade.push(MachineGradeDetail {
                        cpu: grades.cpu,
                        disk: grades.cpu,
                        gpu: grades.gpu,
                        mem: grades.mem,
                        net: grades.net,
                    });

                    appraisal_price.push(machine_info.data.appraisal_price);
                }

                if Self::vec_all_same(&machine_grade) {
                    T::OnlineProfile::add_machine_grades(
                        machine_id.to_vec(),
                        ConfirmedMachine {
                            machine_grade: machine_grade[0],
                            committee_info: vec![],
                        },
                    );
                }

                if Self::vec_all_same(&appraisal_price) {
                    T::OnlineProfile::add_machine_price(machine_id.to_vec(), appraisal_price[0]);
                }

                T::OnlineProfile::rm_bonding_id(machine_id.to_vec());
                T::OnlineProfile::add_booking_item(
                    machine_id.to_vec(),
                    BookingItem {
                        machine_id: machine_id.to_vec(),
                        book_time: <frame_system::Module<T>>::block_number(),
                    },
                );
            }

            Ok(().into())
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        HttpFetchingError,
        // HttpDecodeError,
        IndexOutOfRange,
        MachineURLEmpty,
        OffchainUnsignedTxError,
    }
}

impl<T: Config> Pallet<T> {
    // TODO: 将ss58address转为public key
    // 参考：primitives/core/src/crypto.rs: impl Ss58Codec for AccountId32
    // from_ss58check_with_version
    pub fn wallet_match_account(who: T::AccountId, s: &Vec<u8>) -> bool {
        // const CHECKSUM_LEN: usize = 2;
        let mut data: [u8; 35] = [0; 35];
        let decoded = bs58::decode(s).into(&mut data);

        match decoded {
            Ok(length) => {
                if length != 35 {
                    return false;
                }
            }
            Err(_) => return false,
        }

        let (_prefix_len, _ident) = match data[0] {
            0..=63 => (1, data[0] as u16),
            64..=127 => {
                // let lower = (data[0] << 2) | (data[1] >> 6);
                // let upper = data[1] & 0b00111111;
                // (2, (lower as u16) | ((upper as u16) << 8))
                return false;
            }
            _ => return false,
        };

        let account_id32: [u8; 32] = data[1..33].try_into().unwrap();
        let wallet = T::AccountId::decode(&mut &account_id32[..]).unwrap_or_default();

        if who == wallet {
            return true;
        }
        return false;
    }

    fn vec_all_same<C: PartialEq + Copy>(arr: &[C]) -> bool {
        if arr.is_empty() {
            return true;
        }
        let first = arr[0];
        arr.iter().all(|&item| item == first)
    }

    fn call_ocw_machine_info() -> Result<(), Error<T>> {
        let call = Call::ocw_submit_machine_info();
        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()).map_err(|_| {
            debug::error!("Failed in offchain_unsigned_tx");
            <Error<T>>::OffchainUnsignedTxError
        })
    }

    // 产生一组随机的机器信息URL，并更新到存储
    fn update_machine_info_url() {
        let mut machine_info_url = MachineInfoURL::<T>::get();
        let machine_info_rand_url_num = MachineInfoRandURLNum::<T>::get();
        let mut next_group: Vec<MachineId> = Vec::new();

        if machine_info_url.len() == 0 {
            return;
        }

        if (machine_info_url.len() as u32) < machine_info_rand_url_num {
            MachineInfoRandURL::<T>::put(machine_info_url);
            return;
        }

        for _ in 0..machine_info_rand_url_num {
            // let url_index = T::OnlineProfile::co_random_num(machine_info_url.len() as u32 - 1);
            let url_index = <random_num::Module<T>>::random_u32(machine_info_url.len() as u32 - 1);
            // let url_index = T::OnlineProfile::co_random_num(machine_info_url.len() as u32 - 1);
            // let url_index = Self::random_num(machine_info_url.len() as u32 - 1);
            next_group.push(machine_info_url[url_index as usize].to_vec());
            machine_info_url.remove(url_index as usize);
        }

        MachineInfoRandURL::<T>::put(next_group);
    }

    // 通过http获取机器的信息
    pub fn fetch_machine_info(
        url: &Vec<u8>,
        machine_id: &Vec<u8>,
    ) -> Result<machine_info::MachineInfo, Error<T>> {
        let mut url = url.to_vec();
        url.extend(machine_id.iter());

        let url = str::from_utf8(&url)
            .map_err(|_| http::Error::Unknown)
            .unwrap();
        debug::info!("sending request to: {}", &url);

        let request = offchain::http::Request::get(&url);

        let timeout =
            sp_io::offchain::timestamp().add(offchain::Duration::from_millis(FETCH_TIMEOUT_PERIOD));

        let pending = request
            .add_header("User-Agent", HTTP_HEADER_USER_AGENT)
            .deadline(timeout)
            .send()
            .map_err(|_| <Error<T>>::HttpFetchingError)?;

        let response = pending
            .try_wait(timeout)
            .map_err(|_| <Error<T>>::HttpFetchingError)?
            .map_err(|_| <Error<T>>::HttpFetchingError)?;

        if response.code != 200 {
            debug::error!("Unexpected http request status code: {}", response.code);
            return Err(<Error<T>>::HttpFetchingError);
        }

        let body = response.body().collect::<Vec<u8>>();
        let body_str = sp_std::str::from_utf8(&body)
            .map_err(|_| {
                debug::warn!("No UTF8 body");
                http::Error::Unknown
            })
            .unwrap(); // TODO: handle error here

        let machine_info: machine_info::MachineInfo = serde_json::from_str(&body_str).unwrap(); // TODO: handler error here

        debug::info!("#### MachineInfo str: {}", &body_str);
        debug::info!("############ Machine_info is: {:?}", machine_info);

        Ok(machine_info)
    }
}
