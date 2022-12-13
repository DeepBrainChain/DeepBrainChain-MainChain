use crate::{BalanceOf, Config, Error, MachineId, MachineInfo, Pallet, PosGPUInfo};
use frame_support::{dispatch::DispatchResultWithPostInfo, ensure};
use sp_core::crypto::Public;
use sp_runtime::traits::Verify;
use sp_std::{
    convert::{TryFrom, TryInto},
    str,
    vec::Vec,
};

impl<T: Config> Pallet<T> {
    pub fn pay_fixed_tx_fee(who: T::AccountId) -> DispatchResultWithPostInfo {
        <generic_func::Module<T>>::pay_fixed_tx_fee(who).map_err(|_| Error::<T>::PayTxFeeFailed)?;
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

// Reference： primitives/core/src/crypto.rs: impl Ss58Codec for AccountId32
// from_ss58check_with_version
pub fn get_accountid32(addr: &[u8]) -> Option<[u8; 32]> {
    let mut data: [u8; 35] = [0; 35];

    let length = bs58::decode(addr).into(&mut data).ok()?;
    if length != 35 {
        return None
    }

    let (_prefix_len, _ident) = match data[0] {
        0..=63 => (1, data[0] as u16),
        _ => return None,
    };

    let account_id32: [u8; 32] = data[1..33].try_into().ok()?;
    Some(account_id32)
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
    let public = sp_core::sr25519::Public::from_slice(&account_id32);

    signature.verify(&msg[..], &public).then(|| ())
}

#[allow(dead_code)]
fn get_public_from_str(addr: &[u8]) -> Option<sp_core::sr25519::Public> {
    let account_id32: [u8; 32] = get_accountid32(addr)?;
    Some(sp_core::sr25519::Public::from_slice(&account_id32))
}
