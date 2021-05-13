// 主要功能：
// 1. 从online-profile中读取bonding_machine(需要查询机器信息的机器)
// 2. 设置并随机选择一组API，可供查询: 增加URL，删除URL，设置随机URL个数都会更新这组随机URL；同时每10个块更新一次URL
// 3. 从一组随机的API中查询机器信息，并对比。如果一致，则存储机器的信息，机器信息写回到online_profile (机器信息包括机器得分).

#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{debug, dispatch::DispatchResultWithPostInfo, pallet_prelude::*};
use frame_system::{
    offchain::{CreateSignedTransaction, SubmitTransaction},
    pallet_prelude::*,
};
use online_profile::{machine_info::*, types::*};
use online_profile_machine::{LCOps, OCWOps};
use sp_runtime::{offchain, offchain::http, traits::SaturatedConversion};
use sp_std::{convert::TryInto, prelude::*, str};

pub use pallet::*;

#[cfg(test)]
mod mock;

#[cfg(test)]
mod tests;

pub type MachineId = Vec<u8>;
pub const UNSIGNED_TXS_PRIORITY: u64 = 100;

#[rustfmt::skip]
#[frame_support::pallet]
pub mod pallet {
    use super::*;

    #[pallet::config]
    pub trait Config: frame_system::Config + CreateSignedTransaction<Call<Self>> + online_profile::Config + random_num::Config
    {
        // type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type OnlineProfile: LCOps<MachineId = MachineId>
            + OCWOps<MachineId = MachineId, MachineInfo = online_profile::MachineInfo<Self::AccountId, Self::BlockNumber>>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    // 添加 machineInfoURL, 并进行随机选择一些节点
    // eg: pub MachineInfoUrl get(fn machine_info_url) config(): MachineId = "http://116.85.24.172:41107/api/v1/mining_nodes/".as_bytes().to_vec();
    #[pallet::storage]
    #[pallet::getter(fn machine_info_url)]
    pub(super) type MachineInfoURL<T> = StorageValue<_, Vec<MachineId>, ValueQuery>;

    // 设置最小URL数量。小于该数量，将不开始进行抢单
    #[pallet::type_value]
    pub fn URLNumMinDefault<T: Config>() -> u32 {
        3
    }

    #[pallet::storage]
    #[pallet::getter(fn url_num_min)]
    pub(super) type URLNumMin<T: Config> = StorageValue<_, u32, ValueQuery, URLNumMinDefault<T>>;

    // pub MachineInfoRandURLNum get(fn machine_info_rand_url_num) config(): u32 = 3;
    #[pallet::type_value]
    pub fn MachineInfoRandURLNumDefault<T: Config>() -> u32 {
        3
    }

    #[pallet::storage]
    #[pallet::getter(fn machine_info_rand_url_num)]
    pub(super) type MachineInfoRandURLNum<T: Config> =
        StorageValue<_, u32, ValueQuery, MachineInfoRandURLNumDefault<T>>;

    #[pallet::storage]
    #[pallet::getter(fn request_count)]
    pub(super) type RequestCount<T: Config> = StorageMap<_,Blake2_128Concat, MachineId, u32, ValueQuery>;

    #[pallet::type_value]
    pub fn RequestLimitDefault<T: Config>() -> u32 {
        3
    }

    #[pallet::storage]
    #[pallet::getter(fn request_limit)]
    pub(super) type RequestLimit<T> = StorageValue<_, u32, ValueQuery, RequestLimitDefault<T>>;

    // 验证次数也跟offchain调用验证函数的频率有关
    #[pallet::type_value]
    pub fn VerifyTimesDefault<T: Config>() -> u32 {
        4
    }

    #[pallet::storage]
    #[pallet::getter(fn verify_times)]
    pub(super) type VerifyTimes<T: Config> = StorageValue<_, u32, ValueQuery, VerifyTimesDefault<T>>;

    /// random url for machine info
    #[pallet::storage]
    #[pallet::getter(fn machine_info_rand_url)]
    pub(super) type MachineInfoRandURL<T> = StorageValue<_, Vec<MachineId>, ValueQuery>;

    #[pallet::validate_unsigned]
    impl<T: Config> ValidateUnsigned for Pallet<T> {
        type Call = Call<T>;
        fn validate_unsigned(_source: TransactionSource, call: &Self::Call) -> TransactionValidity {
            let valid_tx = |provide| {
                ValidTransaction::with_tag_prefix("online-profile-ocw")
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

            if Self::url_num_min() < Self::machine_info_url().len() as u32 {
                return
            }

            let result = Self::call_ocw_machine_info();
            if let Err(e) = result {
                debug::error!("offchain_worker error: {:?}", e);
            }
        }

        fn on_finalize(block_number: T::BlockNumber) {
            if block_number.saturated_into::<u64>() / 10 == 0 {
                Self::update_rand_url()
            }
        }
    }

    #[pallet::call]
    impl<T: Config> Pallet<T> {
        // root用户添加机器信息API
        #[pallet::weight(0)]
        pub fn add_machine_info_url(origin: OriginFor<T>, new_url: MachineId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut machine_info_url = MachineInfoURL::<T>::get();
            machine_info_url.push(new_url.clone());
            MachineInfoURL::<T>::put(machine_info_url);

            Self::update_rand_url();
            Ok(().into())
        }

        // root用户删除机器信息API
        #[pallet::weight(0)]
        fn rm_url_by_index(origin: OriginFor<T>, index: u32) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut machine_info_url = MachineInfoURL::<T>::get();
            ensure!(index > machine_info_url.len() as u32, Error::<T>::IndexOutOfRange);
            machine_info_url.remove(index as usize);
            MachineInfoURL::<T>::put(machine_info_url);

            Self::update_rand_url();
            Ok(().into())
        }

        // root用户设置随机选择多少API进行验证机器信息
        #[pallet::weight(0)]
        fn set_machineinfo_rand_url_num(origin: OriginFor<T>, num: u32) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            MachineInfoRandURLNum::<T>::put(num);
            Self::update_rand_url();
            Ok(().into())
        }
        // ocw 实现获取machine info并发送unsigned tx以修改到存储
        // UserBondedMachine增加who-machine_id pair;
        // BondedMachineId 增加 machine_id => ()
        // BondingQueueMachineId 减少 machine_id
        #[pallet::weight(0)]
        fn ocw_submit_machine_info(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;

            // 获取待绑定的id
            let live_machines = <online_profile::Pallet<T>>::live_machines();
            let bonding_queue_id = live_machines.bonding_machine;

            let request_limit = RequestLimit::<T>::get();

            let machine_info_url = MachineInfoRandURL::<T>::get();
            ensure!(machine_info_url.len() != 0, Error::<T>::MachineURLEmpty);

            for machine_id in bonding_queue_id.iter() {
                let mut machine_info = <online_profile::Pallet<T>>::machines_info(&machine_id);
                let mut request_count = Self::request_count(&machine_id);

                if let Some(ocw_machine_info) = Self::machine_info_identical(machine_id) {
                    machine_info.ocw_machine_info = ocw_machine_info.clone();
                    if let Some(machine_grade )= Self::total_min_num(ocw_machine_info.gpu.gpus){
                        machine_info.machine_grade = machine_grade;
                    };

                    T::OnlineProfile::update_machine_info(&machine_id, machine_info);
                    T::OnlineProfile::rm_bonding_id(machine_id.to_vec());
                    T::OnlineProfile::add_ocw_confirmed_id(machine_id.to_vec());
                } else {
                    request_count += 1;
                    if request_count == request_limit { // 已经超过请求次数，从中删除
                        T::OnlineProfile::rm_bonding_id(machine_id.to_vec());
                    }
                    RequestCount::<T>::insert(&machine_id, request_count);
                }
            }

            Ok(().into())
        }

        // #[pallet::weight(0)]
        // fn ocw_submit_online_proof(origin: OriginFor<T>) -> DispatchResultWithPostInfo {
        //     ensure_none(origin)?;

        //     let verify_time = VerifyTimes::<T>::get();

        //     // 首先获取所有机器ID列表
        //     let staking_machine = T::OnlineProfile::staking_machine();
        //     // let mut staking_machine: Vec<_> = staking_machine.collect();
        //     let a = staking_machine.len();
        //     // 然后随机挑选机器
        //     // 验证机器是否在线的信息，并提交
        //     let machine_info_url = MachineInfoRandURL::<T>::get();
        //     ensure!(machine_info_url.len() != 0, Error::<T>::MachineURLEmpty);
        //     // let machine_info = Self::fetch_machine_info(&machine_info_url[0],)
        //     // T::OnlineProfile::add_verify_result();

        //     Ok(().into())
        // }
    }

    // #[pallet::event]
    // #[pallet::metadata(T::AccountId = "AccountId")]
    // #[pallet::generate_deposit(pub(super) fn deposit_event)]
    // pub enum Event<T: Config> {
    //     ReportMachineOffline(T::AccountId, MachineId),
    // }

    #[pallet::error]
    pub enum Error<T> {
        HttpFetchingError,
        // HttpDecodeError,
        IndexOutOfRange,
        MachineURLEmpty,
        OffchainUnsignedTxError,
        // OffchainGradeOrPriceInconsistent,
    }
}

#[rustfmt::skip]
impl<T: Config> Pallet<T> {
    // 参考：primitives/core/src/crypto.rs: impl Ss58Codec for AccountId32
    // from_ss58check_with_version
    pub fn verify_bonding_account(who: T::AccountId, s: &Vec<u8>) -> bool {
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

    fn _vec_identical<C: PartialEq + Copy>(arr: &[C]) -> bool {
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
    fn update_rand_url() {
        let mut machine_info_url = MachineInfoURL::<T>::get();
        let mut next_group: Vec<MachineId> = Vec::new();
        let rand_url_num = MachineInfoRandURLNum::<T>::get();

        if machine_info_url.len() == 0 {
            return;
        }

        if rand_url_num >= (machine_info_url.len() as u32) {
            MachineInfoRandURL::<T>::put(machine_info_url);
        } else {
            for _ in 0..rand_url_num {
                let url_index = <random_num::Module<T>>::random_u32(machine_info_url.len() as u32 - 1);
                next_group.push(machine_info_url[url_index as usize].to_vec());
                machine_info_url.remove(url_index as usize);
            }
            MachineInfoRandURL::<T>::put(next_group);
        }
    }

    // 通过http获取机器的信息
    pub fn fetch_machine_info(url: &Vec<u8>, machine_id: &Vec<u8>) -> Result<MachineInfo, Error<T>> {
        let mut url = url.to_vec();
        url.extend(b"/");
        url.extend(machine_id.iter());

        let url = str::from_utf8(&url).map_err(|_| http::Error::Unknown).unwrap();

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

        let machine_info: MachineInfo = serde_json::from_str(&body_str).unwrap(); // TODO: handler error here

        debug::info!("#### MachineInfo str: {}", &body_str);
        debug::info!("############ Machine_info is: {:?}", machine_info);

        Ok(machine_info)
    }

    // 通过多个URL获取机器信息，如果一致，则验证通过
    fn machine_info_identical(id: &MachineId) -> Option<OCWMachineInfo> {
        let info_url = Self::machine_info_rand_url();

        let mut machine_info = Vec::new();

        for url in info_url.iter() {
            let ocw_machine_info = Self::fetch_machine_info(&url, id);
            if let Err(e) = ocw_machine_info {
                debug::error!("fetch_machine_info failed: {:?}", e);
                return None;
            }
            let ocw_machine_info = ocw_machine_info.unwrap();
            if ocw_machine_info.data.wallet.len() != 1 {
                return None;
            }

            let tmp_info = OCWMachineInfo {
                cpu: ocw_machine_info.data.cpu,
                disk: ocw_machine_info.data.disk,
                gpu: ocw_machine_info.data.gpu,
                ip: ocw_machine_info.data.ip,
                mem: ocw_machine_info.data.mem,
                os: ocw_machine_info.data.os,
                version: ocw_machine_info.data.version,
            };

            if machine_info.len() == 0 {
                machine_info.push(tmp_info);
            } else if machine_info[0] != tmp_info {
                debug::error!("Machine info must be identical");
                return None;
            }
        }

        return Some(machine_info[0].clone());
    }

    fn total_min_num(gpus: Vec<GPUDetail>) -> Option<u64> {
        // FIXME
        // let mut grade_out = Vec::new();
        // for a_gpu_detail in gpus.iter() {
        //     if let Some(a_grade) = Self::vec_u8_to_u64(&a_gpu_detail.grade) {
        //         grade_out.push(a_grade);
        //     };
        // }
        // if grade_out.len() == 0 {
        //     return None;
        // }

        // return Some(grade_out.iter().min().unwrap() * grade_out.len() as u64);
        return Some(1);
    }

    fn vec_u8_to_u64(num_str: &Vec<u8>) -> Option<u64> {
        let num_str = str::from_utf8(num_str);
        if let Err(_e) = num_str {
            return None;
        }
        let num_out: u64 = match num_str.unwrap().parse() {
            Ok(num) => num,
            Err(_e) => return None,
        };

        return Some(num_out);
    }
}
