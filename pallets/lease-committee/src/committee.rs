use codec::{Decode, Encode};
use sp_runtime::RuntimeDebug;
use sp_std::{collections::btree_map::BTreeMap, prelude::Vec};

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct Committee<AccountId, Balance> {
    max_candidacy: u32,
    black_list: Vec<AccountId>,
    candidacy: Vec<AccountId>,
    min_stake: Balance, // TODO: 可能删除掉该变量
                        // Committee: Vec<AccountId>,
}

impl<AccountId, Balance> Committee<AccountId, Balance>
where
    AccountId: Ord + Copy,
{
    // 不在黑名单中, 且候选名单没有满额, 返回true,
    // 如果在黑名单或委员会长度超过限制，否则返回false.
    pub fn add_candidacy(&mut self, member: AccountId) -> bool {
        if let Ok(_) = self.black_list.binary_search(&member) {
            return false;
        }

        if self.candidacy.len() >= self.max_candidacy as usize {
            return false;
        }

        if let Err(index) = self.candidacy.binary_search(&member) {
            self.candidacy.insert(index, member);
        }

        return true;
    }

    // 如果存在于候选人中，直接删除
    pub fn rm_candidacy(&mut self, member: AccountId) {
        if let Ok(index) = self.candidacy.binary_search(&member) {
            self.candidacy.remove(index);
        }
    }

    // 如果在委员会中，将其删除, 并将其从中删除，并插入到黑名单中
    pub fn add_black_list(&mut self, member: AccountId) {
        if let Ok(_) = self.candidacy.binary_search(&member) {
            self.rm_candidacy(member.clone());
        }

        if let Err(index) = self.black_list.binary_search(&member) {
            self.black_list.insert(index, member);
        }
    }

    pub fn rm_black_list(&mut self, member: AccountId) {
        if let Ok(index) = self.black_list.binary_search(&member) {
            self.black_list.remove(index);
        }
    }
}

// TODO: 移动到global变量
// online_book_limit: u32,
// offline_book_limit: u32,
// reonline_book_limit: u32,

// TODO: 将要移动到online_profile 存储中
#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct MachineVerify<AccountId: Ord, BlockNumber> {
    bookable: bool, // 该机器是否可订阅

    online_commit: BTreeMap<AccountId, VerifyResult<BlockNumber>>,

    // TODO: 添加机器不在线时间
    reporter: AccountId,
    offline_commit: BTreeMap<AccountId, VerifyResult<BlockNumber>>,

    reonline_commit: BTreeMap<AccountId, VerifyResult<BlockNumber>>,
}

#[derive(PartialEq, Eq, Clone, Encode, Decode, Default, RuntimeDebug)]
pub struct VerifyResult<BlockNumber> {
    start_height: BlockNumber,
    end_height: BlockNumber,

    verify_hash: Vec<u8>,
    verify_raw: Vec<u8>,
    verify_result: bool,
}
