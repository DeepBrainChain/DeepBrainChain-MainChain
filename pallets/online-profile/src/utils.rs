use sp_core::crypto::Public;
use sp_runtime::traits::Verify;
use sp_std::{
    convert::{TryFrom, TryInto},
    str,
    vec::Vec,
};

// Reference： primitives/core/src/crypto.rs: impl Ss58Codec for AccountId32
// from_ss58check_with_version
pub fn get_accountid32(addr: &[u8]) -> Option<[u8; 32]> {
    let mut data: [u8; 35] = [0; 35];

    let length = bs58::decode(addr).into(&mut data).ok()?;
    if length != 35 {
        return None;
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
    let pubkey_hex: Result<Vec<u8>, _> =
        (0..pubkey_str.len()).step_by(2).map(|i| u8::from_str_radix(&pubkey_str[i..i + 2], 16)).collect();
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
