use crate::{BalanceOf, Config, Error, MachineId, Pallet, PosGPUInfo};
use dbc_support::{
    machine_info::MachineInfo, verify_slash::OPSlashReason, FIVE_DAYS, FOUR_HOURS, ONE_DAY,
    SEVEN_MINUTES, TEN_DAYS, TWO_DAYS,
};
use frame_support::{dispatch::DispatchResultWithPostInfo, ensure};
use sp_core::crypto::ByteArray;
use sp_runtime::{traits::Verify, SaturatedConversion};
use sp_std::{
    convert::{TryFrom, TryInto},
    str,
    vec::Vec,
};

impl<T: Config> Pallet<T> {
    pub fn pay_fixed_tx_fee(who: T::AccountId) -> DispatchResultWithPostInfo {
        <generic_func::Pallet<T>>::pay_fixed_tx_fee(who).map_err(|_| Error::<T>::PayTxFeeFailed)?;
        Ok(().into())
    }

    pub fn check_bonding_msg(
        stash: T::AccountId,
        machine_id: MachineId,
        msg: Vec<u8>,
        sig: Vec<u8>,
    ) -> DispatchResultWithPostInfo {
        // 验证msg: len(machine_id + stash_account) = 64 + 48
        ensure!(msg.len() == 112, Error::<T>::BadMsgLen);

        let (sig_machine_id, sig_stash_account) = (msg[..64].to_vec(), msg[64..].to_vec());
        ensure!(machine_id == sig_machine_id, Error::<T>::SigMachineIdNotEqualBondedMachineId);
        let sig_stash_account = Self::get_account_from_str(&sig_stash_account)
            .ok_or(Error::<T>::ConvertMachineIdToWalletFailed)?;
        ensure!(sig_stash_account == stash, Error::<T>::MachineStashNotEqualControllerStash);

        // 验证签名是否为MachineId发出
        ensure!(verify_sig(msg, sig, machine_id).is_some(), Error::<T>::BadSignature);
        Ok(().into())
    }

    /// GPU online/offline
    // - Writes: PosGPUInfo
    // NOTE: pos_gpu_info only record actual machine grades(reward grade not included)
    pub fn update_region_on_online_changed(
        machine_info: &MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        is_online: bool,
    ) {
        let longitude = machine_info.longitude();
        let latitude = machine_info.latitude();
        let gpu_num = machine_info.gpu_num();
        let calc_point = machine_info.calc_point();

        PosGPUInfo::<T>::mutate(longitude, latitude, |region_mining_power| {
            region_mining_power.on_online_changed(is_online, gpu_num, calc_point);
        });
    }

    pub fn update_region_on_exit(
        machine_info: &MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
    ) {
        let longitude = machine_info.longitude();
        let latitude = machine_info.latitude();
        let gpu_num = machine_info.gpu_num();
        let calc_point = machine_info.calc_point();

        let mut region_mining_power = Self::pos_gpu_info(longitude, latitude);

        let is_empty = region_mining_power.on_machine_exit(gpu_num, calc_point);
        if is_empty {
            PosGPUInfo::<T>::remove(longitude, latitude);
        } else {
            PosGPUInfo::<T>::insert(longitude, latitude, region_mining_power);
        }
    }

    /// GPU rented/surrender
    // - Writes: PosGPUInfo
    pub fn update_region_on_rent_changed(
        machine_info: &MachineInfo<T::AccountId, T::BlockNumber, BalanceOf<T>>,
        is_rented: bool,
    ) {
        let longitude = machine_info.longitude();
        let latitude = machine_info.latitude();
        let gpu_num = machine_info.gpu_num();

        PosGPUInfo::<T>::mutate(longitude, latitude, |region_mining_power| {
            region_mining_power.on_rent_changed(is_rented, gpu_num);
        });
    }
}

// [u8; 64] -> str -> [u8; 32] -> pubkey
pub fn verify_sig(msg: Vec<u8>, sig: Vec<u8>, account: Vec<u8>) -> Option<()> {
    let signature = sp_core::sr25519::Signature::try_from(&sig[..]).ok()?;
    // let public = Self::get_public_from_str(&account)?;

    let pubkey_str = str::from_utf8(&account).ok()?;
    let pubkey_hex: Result<Vec<u8>, _> = (0..pubkey_str.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&pubkey_str[i..i + 2], 16))
        .collect();
    let pubkey_hex = pubkey_hex.ok()?;

    let account_id32: [u8; 32] = pubkey_hex.try_into().ok()?;
    let public = sp_core::sr25519::Public::from_slice(&account_id32).ok()?;

    signature.verify(&msg[..], &public).then(|| ())
}

#[allow(dead_code)]
fn get_public_from_str(addr: &[u8]) -> Option<sp_core::sr25519::Public> {
    let account_id32: [u8; 32] = dbc_support::utils::get_accountid32(addr)?;
    Some(sp_core::sr25519::Public::from_slice(&account_id32).ok()?)
}

impl<T: Config> Pallet<T> {
    // 根据下线时长确定 slash 比例.
    pub fn slash_percent(
        slash_reason: &OPSlashReason<T::BlockNumber>,
        duration: T::BlockNumber,
    ) -> u32 {
        let duration = duration.saturated_into::<u32>();

        match slash_reason {
            OPSlashReason::RentedReportOffline(_) => match duration {
                0..SEVEN_MINUTES => 2,        // <=7M扣除2%质押币。100%进入国库
                SEVEN_MINUTES..TWO_DAYS => 4, // <=48H扣除4%质押币。100%进入国库
                TWO_DAYS..FIVE_DAYS => 30,    // <=120H扣30%质押币，10%给用户，90%进入国库
                _ => 50,                      // >120H扣除50%质押币。10%给用户，90%进入国库
            },
            OPSlashReason::OnlineReportOffline(_) => match duration {
                // TODO: 如果机器从首次上线时间起超过365天，剩下20%押金可以申请退回。扣除80%质押币。
                // 质押币全部进入国库。
                0..SEVEN_MINUTES => 2, /* <=7M扣除2%质押币，全部进入国库。 */
                SEVEN_MINUTES..TWO_DAYS => 4, /* <=48H扣除4%质押币，全部进入国库 */
                TWO_DAYS..TEN_DAYS => 30, /* <=240H扣除30%质押币，全部进入国库 */
                _ => 80,
            },
            OPSlashReason::RentedInaccessible(_) => match duration {
                0..SEVEN_MINUTES => 4,        // <=7M扣除4%质押币。10%给验证人，90%进入国库
                SEVEN_MINUTES..TWO_DAYS => 8, // <=48H扣除8%质押币。10%给验证人，90%进入国库
                TWO_DAYS..FIVE_DAYS => 60, /* <=120H扣除60%质押币。10%给用户，20%给验证人，70%进入国库 */
                _ => 100, /* >120H扣除100%押金。10%给用户，20%给验证人，70%进入国库 */
            },
            OPSlashReason::RentedHardwareMalfunction(_) => match duration {
                0..FOUR_HOURS => 6,        // <=4H扣除6%质押币
                FOUR_HOURS..ONE_DAY => 12, // <=24H扣除12%质押币
                ONE_DAY..TWO_DAYS => 16,   // <=48H扣除16%质押币
                TWO_DAYS..FIVE_DAYS => 60, // <=120H扣除60%质押币
                _ => 100,                  // >120H扣除100%质押币
            },
            OPSlashReason::RentedHardwareCounterfeit(_) => match duration {
                0..FOUR_HOURS => 12,       // <=4H扣12%质押币
                FOUR_HOURS..ONE_DAY => 24, // <=24H扣24%质押币
                ONE_DAY..TWO_DAYS => 32,   // <=48H扣32%质押币
                TWO_DAYS..FIVE_DAYS => 60, // <=120H扣60%质押币
                _ => 100,                  // >120H扣100%押金
            },
            OPSlashReason::OnlineRentFailed(_) => match duration {
                0..FOUR_HOURS => 6,        // <=4H扣6%质押币
                FOUR_HOURS..ONE_DAY => 12, // <=24H扣12%质押币
                ONE_DAY..TWO_DAYS => 16,   // <=48H扣16%质押币
                TWO_DAYS..FIVE_DAYS => 60, // <=120H扣60%质押币
                _ => 100,                  // >120H扣100%押金
            },
            _ => 0,
        }
    }
}

pub fn reach_max_slash<BlockNumber>(
    slash_reason: &OPSlashReason<BlockNumber>,
    duration: u64,
) -> bool {
    let max_slash = |threshold| {
        if duration > threshold {
            true
        } else {
            false
        }
    };

    match slash_reason {
        OPSlashReason::RentedReportOffline(_) => max_slash(5 * ONE_DAY as u64),
        OPSlashReason::OnlineReportOffline(_) => max_slash(10 * ONE_DAY as u64),
        OPSlashReason::RentedInaccessible(_) => max_slash(5 * ONE_DAY as u64),
        OPSlashReason::RentedHardwareMalfunction(_) => max_slash(5 * ONE_DAY as u64),
        OPSlashReason::RentedHardwareCounterfeit(_) => max_slash(5 * ONE_DAY as u64),
        OPSlashReason::OnlineRentFailed(_) => max_slash(5 * ONE_DAY as u64),
        _ => false,
    }
}
