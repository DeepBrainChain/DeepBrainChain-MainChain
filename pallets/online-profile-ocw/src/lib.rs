// 主要功能：
// 1. 从online-profile中读取bonding_machine(需要查询机器信息的机器)
// 2. 设置并随机选择一组API，可供查询: 增加URL，删除URL，设置随机URL个数都会更新这组随机URL；同时
//    每10个块更新一次URL
// 3. 从一组随机的API中查询机器信息，并对比。通过之后，则存储机器的信息，机器信息写回到online_profile.
// ~~4. 委员会需要根据信息，填写根据一组简单的公式，填写机器的得分~~

#![recursion_limit = "256"]
#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::{debug, dispatch::DispatchResultWithPostInfo, pallet_prelude::*};
use frame_system::{
    offchain::{CreateSignedTransaction, SubmitTransaction},
    pallet_prelude::*,
};
use online_profile::machine_info::*;
use online_profile::types::*;
use online_profile_machine::{LCOps, OCWOps};
use sp_runtime::{offchain, offchain::http, traits::SaturatedConversion};
use sp_std::{convert::TryInto, prelude::*, str};

// pub mod machine_info;

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
    pub trait Config:
        frame_system::Config
        + CreateSignedTransaction<Call<Self>>
        + online_profile::Config
        + random_num::Config
    {
        // type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
        type OnlineProfile: LCOps<MachineId = MachineId>
            + OCWOps<
                MachineId = MachineId,
                MachineInfo = online_profile::MachineInfo<Self::AccountId, Self::BlockNumber>,
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

    // 验证次数也跟offchain调用验证函数的频率有关
    #[pallet::type_value]
    pub fn VerifyTimesDefault<T: Config>() -> u32 {
        4
    }

    #[pallet::storage]
    #[pallet::getter(fn verify_times)]
    pub(super) type VerifyTimes<T: Config> =
        StorageValue<_, u32, ValueQuery, VerifyTimesDefault<T>>;

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
        fn add_machine_info_url(origin: OriginFor<T>, new_url: MachineId) -> DispatchResultWithPostInfo {
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

            let live_machines = <online_profile::Pallet<T>>::live_machines();
            let bonding_queue_id = live_machines.bonding_machine;

            // TODO: 当查询成功之后，必须要将bonding_queue_id 从 bonding_machine变量中移除
            let request_limit = RequestLimit::<T>::get();
            let machine_info_url = MachineInfoRandURL::<T>::get();

            ensure!(machine_info_url.len() != 0, Error::<T>::MachineURLEmpty);

            for machine_id in bonding_queue_id.iter() {
                let mut machine_info = <online_profile::Pallet<T>>::machines_info(&machine_id);

                // 该machine_id请求次数已经超过了限制
                if machine_info.bonding_requests > request_limit {
                    debug::info!("machine_id: {:?} has reached request limit", &machine_id);
                    continue;
                }

                if let Some(machine_info) = Self::machine_info_identical(machine_id) {
                    // TODO： 将ocw获取到的机器信息记录到onlineprofile模块
                    // T::OnlineProfile::update_machine_info(&machine_id, machine_info);
                } else {
                    machine_info.bonding_requests += 1;
                }

                // 机器ID完成了一次OCW请求之后，应该+1
                machine_info.bonding_requests += 1;

                T::OnlineProfile::update_machine_info(&machine_id, machine_info);

                // for url in machine_info_url.iter() {
                //     let ocw_machine_info = Self::fetch_machine_info(&url, &machine_id);
                //     if let Err(e) = ocw_machine_info {
                //         // TODO: handle 404的情况(machine_id not found)
                //         debug::error!("Offchain worker error: {:?}", e);
                //         continue;
                //     }

                //     let ocw_machine_info = ocw_machine_info.unwrap();
                //     let machine_wallet = &ocw_machine_info.data.wallet[1].0;

                //     debug::info!("machine wallet is: {:?}", &machine_wallet);

                //     // 如果钱包不一致，则直接进行下一个machine_id的查询
                //     if !Self::verify_bonding_account(
                //         machine_info.machine_owner.clone(),
                //         machine_wallet,
                //     ) {
                //         // TODO: 增加log提示
                //         // T::BondingQueue::<T>::remove(machine_id);
                //         debug::error!(
                //             "OCW bonding: user account {:?} not match machine wallet {:?}, will remove",
                //             &machine_info.machine_owner,
                //             &machine_wallet
                //         );
                //         // 当达到request limit时，删除掉
                //         // T::OnlineProfile::rm_bonding_id(machine_id.to_vec());
                //         break;
                //     }

                //     // TODO: 转为数字
                //     let a_gpu_num = str::from_utf8(&ocw_machine_info.data.gpu.num);
                //     if let Err(e) = a_gpu_num {
                //         debug::error!("Convert u8 to str failed: {:?}", e);
                //         continue;
                //     }
                //     let a_gpu_num = a_gpu_num.unwrap();
                //     let a_gpu_num: u32 = match a_gpu_num.parse() {
                //         Ok(num) => num,
                //         Err(e) => {
                //             debug::error!("Convert str to u32 failed: {:?}", e);
                //             continue;
                //         }
                //     };

                //     gpu_num.push(a_gpu_num);
                // }

            }

            Ok(().into())
        }

        // 用于提交机器是否在线的交易
        // TODO: 当接收到机器不在线的交易后，OCW开启工作，进行验证机器在线信息
        #[pallet::weight(0)]
        fn submit_machine_online(origin: OriginFor<T>, machine_id: MachineId) -> DispatchResultWithPostInfo {
            let reposter = ensure_signed(origin)?;
            let report_time = <frame_system::Module<T>>::block_number();

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

// TODO: 是否需要重新定义

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
                let url_index =
                    <random_num::Module<T>>::random_u32(machine_info_url.len() as u32 - 1);
                next_group.push(machine_info_url[url_index as usize].to_vec());
                machine_info_url.remove(url_index as usize);
            }
            MachineInfoRandURL::<T>::put(next_group);
        }
    }

    // 通过http获取机器的信息
    pub fn fetch_machine_info(
        url: &Vec<u8>,
        machine_id: &Vec<u8>,
    ) -> Result<MachineInfo, Error<T>> {
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

        let machine_info: MachineInfo = serde_json::from_str(&body_str).unwrap(); // TODO: handler error here

        debug::info!("#### MachineInfo str: {}", &body_str);
        debug::info!("############ Machine_info is: {:?}", machine_info);

        Ok(machine_info)
    }

    // 通过多个URL获取机器信息，如果一致，则验证通过
    fn machine_info_identical(id: &MachineId) -> Option<OCWMachineInfo> {
        // 首先获取到MachineId
        let info_url = Self::machine_info_rand_url();

        let mut machine_info = Vec::new();

        for url in info_url.iter() {
            let ocw_machine_info = Self::fetch_machine_info(&url, id);
            if let Err(e) = ocw_machine_info {
                debug::error!("fetch_machine_info failed: {:?}", e);
                return None;
            }
            let ocw_machine_info = ocw_machine_info.unwrap();
            let machine_wallets = &ocw_machine_info.data.wallet;
            if machine_wallets.len() != 1 {
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
}
