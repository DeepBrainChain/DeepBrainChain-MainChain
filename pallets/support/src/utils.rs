use sp_runtime::traits::Verify;
use sp_std::{
    convert::{TryFrom, TryInto},
    str, vec,
    vec::Vec,
};

// Reference： primitives/core/src/crypto.rs: impl Ss58Codec for AccountId32
// from_ss58check_with_version
// eg.
// let account: Vec<u8> = b"5GR31fgcHdrJ14eFW1xJmHhZJ56eQS7KynLKeXmDtERZTiw2".to_vec();
// let account_id32: [u8; 32] = Self::get_accountid32(&treasury).unwrap_or_default();
// let account = T::AccountId::decode(&mut &account_id32[..]).ok().unwrap_or_default();
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
    let public = sp_core::sr25519::Public::from_raw(account_id32);

    signature.verify(&msg[..], &public).then(|| ())
}

#[allow(dead_code)]
fn get_public_from_str(addr: &[u8]) -> Option<sp_core::sr25519::Public> {
    let account_id32: [u8; 32] = get_accountid32(addr)?;
    Some(sp_core::sr25519::Public::from_raw(account_id32))
}

use sp_io::hashing::blake2_128;
pub fn get_hash(raw_str: Vec<Vec<u8>>) -> [u8; 16] {
    let mut full_str = Vec::new();
    for a_str in raw_str {
        full_str.extend(a_str);
    }
    blake2_128(&full_str)
}

use crate::{
    machine_type::CommitteeUploadInfo,
    verify_online::{OCMachineCommitteeList, Summary, VerifyResult},
    ItemList,
};
use sp_std::{collections::btree_set::BTreeSet, ops};
pub trait OnlineCommitteeSummary {
    //<AccountId, BlockNumber> {
    type AccountId;
    type BlockNumber;

    // 总结机器的确认情况: 检查机器是否被确认，并检查提交的信息是否一致
    // 返回三种状态：
    // 1. 无共识：处理办法：退还委员会质押，机器重新派单。
    // 2. 支持上线: 处理办法：扣除所有反对上线，支持上线但提交无效信息的委员会的质押。
    // 3. 反对上线: 处理办法：反对的委员会平分支持的委员会的质押。扣5%矿工质押，
    // 允许矿工再次质押而上线。
    fn summary_confirmation(
        machine_committee: OCMachineCommitteeList<Self::AccountId, Self::BlockNumber>,
        committee_submit_info: Vec<CommitteeUploadInfo>,
    ) -> Summary<Self::AccountId>
    where
        Self::AccountId: Clone + Ord + Default,
        Self::BlockNumber: Copy + PartialOrd + ops::Add<Output = Self::BlockNumber> + From<u32>,
    {
        // 如果是反对上线，则需要忽略其他字段，只添加is_support=false的字段
        let mut submit_info = vec![];
        committee_submit_info.into_iter().for_each(|info| {
            if info.is_support {
                submit_info.push(info);
            } else {
                submit_info.push(CommitteeUploadInfo { is_support: false, ..Default::default() })
            }
        });

        let mut summary = Summary::default();
        summary.unruly = machine_committee.summary_unruly();
        let uniq_len = submit_info.iter().collect::<BTreeSet<_>>().len();

        if machine_committee.confirmed_committee.is_empty() {
            // Case: Zero info
            summary.verify_result = VerifyResult::NoConsensus;
        } else if submit_info.iter().min().unwrap() == submit_info.iter().max().unwrap() {
            // Cases: One submit info; Two same info; Three same info
            let info = submit_info[0].clone();
            if info.is_support {
                summary.info = Some(info);
                summary.valid_vote = machine_committee.confirmed_committee;
                summary.verify_result = VerifyResult::Confirmed;
            } else {
                summary.valid_vote = machine_committee.confirmed_committee;
                summary.verify_result = VerifyResult::Refused;
            }
        } else if uniq_len == submit_info.len() {
            // Cases: Two different info; Three different info
            summary.invalid_vote = machine_committee.confirmed_committee;
            summary.verify_result = VerifyResult::NoConsensus;
        } else {
            // Cases: Three info: two is same.
            let (valid_1, valid_2, invalid) = if submit_info[0] == submit_info[1] {
                (0, 1, 2)
            } else if submit_info[0] == submit_info[2] {
                (0, 2, 1)
            } else {
                (1, 2, 0)
            };

            summary.invalid_vote = vec![machine_committee.confirmed_committee[invalid].clone()];
            if submit_info[valid_1].is_support {
                summary.info = Some(submit_info[valid_1].clone());
                summary.valid_vote = vec![
                    machine_committee.confirmed_committee[valid_1].clone(),
                    machine_committee.confirmed_committee[valid_2].clone(),
                ];
                summary.verify_result = VerifyResult::Confirmed;
            } else {
                ItemList::expand_to_order(
                    &mut summary.valid_vote,
                    vec![
                        machine_committee.confirmed_committee[valid_1].clone(),
                        machine_committee.confirmed_committee[valid_2].clone(),
                    ],
                );
                summary.verify_result = VerifyResult::Refused;
            }
        };
        summary
    }
}
