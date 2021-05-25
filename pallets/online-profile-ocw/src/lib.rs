// 主要功能：
// 1. 从online-profile中读取bonding_machine(需要查询机器信息的机器)
// 2. 设置并随机选择一组API，可供查询: 增加URL，删除URL，设置随机URL个数都会更新这组随机URL；同时每10个块更新一次URL
// 3. 从一组随机的API中查询机器信息，并对比。如果一致，则存储机器的信息，机器信息写回到online_profile (机器信息包括机器得分).
// 4. 只需要存储机器ID--钱包地址即可，其他信息由委员会提交

#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{debug, dispatch::DispatchResultWithPostInfo, pallet_prelude::*};
use frame_system::{
    offchain::{CreateSignedTransaction, SubmitTransaction},
    pallet_prelude::*,
};
use online_profile::types::*;
use online_profile_machine::OCWOps;
use sp_runtime::{offchain, traits::SaturatedConversion};
use sp_std::{convert::TryInto, prelude::*, str};

mod machine_info;

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
        type OnlineProfile: OCWOps<MachineId = MachineId, AccountId = Self::AccountId>;
    }

    #[pallet::pallet]
    #[pallet::generate_store(pub(super) trait Store)]
    pub struct Pallet<T>(_);

    // 添加 machineInfoURL, 并进行随机选择一些节点
    // eg: pub MachineInfoUrl get(fn machine_info_url) config(): MachineId = "http://116.85.24.172:41107/api/v1/mining_nodes/".as_bytes().to_vec();
    #[pallet::storage]
    #[pallet::getter(fn machine_info_url)]
    pub(super) type MachineInfoURL<T> = StorageValue<_, Vec<MachineId>, ValueQuery>;

    // 设置最小多少URL进行验证。小于该数量，该模块将不开始进行验证
    #[pallet::type_value]
    pub fn RandURLNumDefault<T: Config>() -> u32 {
        1
    }

    #[pallet::storage]
    #[pallet::getter(fn rand_url_num)]
    pub(super) type RandURLNum<T: Config> = StorageValue<_, u32, ValueQuery, RandURLNumDefault<T>>;

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
                Call::ocw_submit_machine_info(_machine_id, _machine_bonded_wallet) => valid_tx(b"ocw_submit_machine_info".to_vec()),
                _ => InvalidTransaction::Call.into(),
            }
        }
    }

    #[pallet::hooks]
    impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {
        fn offchain_worker(block_number: T::BlockNumber) {
            debug::info!("Entering off-chain worker, at height: {:?}", block_number);

            // 检查有足够的URL
            if Self::rand_url_num() < Self::machine_info_url().len() as u32 {
                return
            }

            let bookable_machine = <online_profile::Pallet<T>>::ocw_booking_machine();
            for machine_id in bookable_machine.iter() {
                // let ocw_machine_info = Self::machine_info_identical(machine_id);
                let machine_bonded_wallet = Self::get_machine_info_identical_wallet(machine_id);

                let result = Self::call_ocw_machine_info(machine_id.to_vec(), machine_bonded_wallet);
                if let Err(e) = result {
                    debug::error!("offchain_worker error: {:?}", e);
                }
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
        // 用户添加机器信息API
        #[pallet::weight(0)]
        pub fn add_machine_info_url(origin: OriginFor<T>, new_url: MachineId) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut machine_info_url = Self::machine_info_url();
            machine_info_url.push(new_url);
            MachineInfoURL::<T>::put(machine_info_url);
            Self::update_rand_url();
            Ok(().into())
        }

        // 用户删除机器信息API
        #[pallet::weight(0)]
        fn rm_url_by_index(origin: OriginFor<T>, index: u32) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            let mut machine_info_url = Self::machine_info_url();
            ensure!(index < machine_info_url.len() as u32, Error::<T>::IndexOutOfRange);
            machine_info_url.remove(index as usize);
            MachineInfoURL::<T>::put(machine_info_url);
            Self::update_rand_url();
            Ok(().into())
        }

        // root用户设置随机选择多少API进行验证机器信息
        #[pallet::weight(0)]
        fn set_rand_url_num(origin: OriginFor<T>, num: u32) -> DispatchResultWithPostInfo {
            ensure_root(origin)?;
            RandURLNum::<T>::put(num);
            Self::update_rand_url();
            Ok(().into())
        }

        // ocw 实现获取machine info并发送unsigned tx以修改到存储
        // UserBondedMachine增加who-machine_id pair;
        // BondedMachineId 增加 machine_id => ()
        // BondingQueueMachineId 减少 machine_id
        #[pallet::weight(0)]
        fn ocw_submit_machine_info(origin: OriginFor<T>, machine_id: MachineId ,machine_bonded_wallet: Option<Vec<u8>>) -> DispatchResultWithPostInfo {
            ensure_none(origin)?;

            let request_limit = Self::request_limit();
            let mut request_count = Self::request_count(&machine_id);

            if let Some(machine_bonded_wallet) = machine_bonded_wallet {
                if let Some(wallet_addr) = Self::get_account_from_str(&machine_bonded_wallet) {
                    T::OnlineProfile::rm_booked_id(&machine_id);
                    T::OnlineProfile::add_ocw_confirmed_id(machine_id.to_vec(), wallet_addr);
                } else {
                    request_count += 1;
                };
            } else {
                request_count += 1;
            }

            // 已经超过请求次数，从中删除
            if request_count == request_limit {
                T::OnlineProfile::rm_booked_id(&machine_id);
            }
            RequestCount::<T>::insert(&machine_id, request_count);

            Ok(().into())
        }
    }

    #[pallet::error]
    pub enum Error<T> {
        HttpFetchingError,
        HttpURLParseError,
        HttpReadBodyError,
        HttpUnmarshalBodyError,
        // HttpDecodeError,
        IndexOutOfRange,
        MachineURLEmpty,
        OffchainUnsignedTxError,
        // OffchainGradeOrPriceInconsistent,
    }
}

#[rustfmt::skip]
impl<T: Config> Pallet<T> {
    fn call_ocw_machine_info(machine_id: MachineId, machine_bonded_wallet: Option<Vec<u8>>) -> Result<(), Error<T>> {
        let call = Call::ocw_submit_machine_info(machine_id.to_vec(), machine_bonded_wallet);
        SubmitTransaction::<T, Call<T>>::submit_unsigned_transaction(call.into()).map_err(|_| {
            debug::error!("Failed in offchain_unsigned_tx");
            <Error<T>>::OffchainUnsignedTxError
        })
    }

    // 参考：primitives/core/src/crypto.rs: impl Ss58Codec for AccountId32
    // from_ss58check_with_version
    fn get_account_from_str(addr: &Vec<u8>) -> Option<T::AccountId> {
        let mut data: [u8; 35] = [0; 35];

        if let Ok(length) = bs58::decode(addr).into(&mut data) {
            if length != 35 {
                return None;
            }
        } else {
            return None;
        }

        let (_prefix_len, _ident) = match data[0] {
            0..=63 => (1, data[0] as u16),
            _ => return None,
        };

        let account_id32: Result<[u8; 32], _> = data[1..33].try_into();
        if let Err(_) = account_id32 {
            return None;
        }

        let account_id32 = account_id32.unwrap();
        if let Ok(wallet)= T::AccountId::decode(&mut &account_id32[..]) {
            return Some(wallet);
        };
        return None;
    }

    // 产生一组随机的机器信息URL，并更新到存储
    fn update_rand_url() {
        let mut machine_info_url = MachineInfoURL::<T>::get();
        let mut next_group: Vec<MachineId> = Vec::new();
        let rand_url_num = Self::rand_url_num();

        if machine_info_url.len() == 0 {
            return;
        }

        if rand_url_num >= (machine_info_url.len() as u32) {
            MachineInfoRandURL::<T>::put(machine_info_url);
        } else {
            for _ in 0..rand_url_num {
                let url_index =
                    <random_num::Module<T>>::random_u32(machine_info_url.len() as u32 - 1);
                next_group.push(machine_info_url[url_index as usize].to_vec());
                machine_info_url.remove(url_index as usize);
            }
            MachineInfoRandURL::<T>::put(next_group);
        }
    }

    // 通过http获取机器的信息
    pub fn fetch_machine_info(url: &Vec<u8>, machine_id: &Vec<u8>) -> Result<machine_info::MachineInfo, Error<T>> {
        let mut url = url.to_vec();
        url.extend(b"/");
        url.extend(machine_id.iter());

        let url = str::from_utf8(&url).map_err(|_| <Error<T>>::HttpURLParseError)?;

        debug::info!("sending request to: {}", &url);

        let request = offchain::http::Request::get(&url);
        let timeout = sp_io::offchain::timestamp().add(offchain::Duration::from_millis(FETCH_TIMEOUT_PERIOD));

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
        let body_str = sp_std::str::from_utf8(&body).map_err(|_| {
            debug::warn!("No UTF8 body");
            <Error<T>>::HttpReadBodyError
        })?;

        return serde_json::from_str(&body_str).map_err(|_| {
            debug::warn!("json unmarshal failed");
            <Error<T>>::HttpUnmarshalBodyError
        });
    }

    // 通过多个URL获取机器信息，如果一致，则验证通过
    // 返回验证一致的钱包地址
    fn get_machine_info_identical_wallet(id: &MachineId) -> Option<Vec<u8>> {
        let info_url = Self::machine_info_rand_url();
        let mut machine_wallet = Vec::new();

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

            if machine_wallet.len() == 0 {
                machine_wallet.push(ocw_machine_info.data.wallet)
            } else if machine_wallet[0] != ocw_machine_info.data.wallet {
                return None;
            }
        }
        return Some(machine_wallet[0].clone());
    }

    fn _vec_u8_to_u64(num_str: &Vec<u8>) -> Option<u64> {
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

    fn _vec_identical<C: PartialEq + Copy>(arr: &[C]) -> bool {
        if arr.is_empty() {
            return true;
        }
        let first = arr[0];
        arr.iter().all(|&item| item == first)
    }
}
