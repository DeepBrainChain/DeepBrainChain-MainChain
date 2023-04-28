#![warn(unused_crate_dependencies)]

use codec::Codec;
use committee::CommitteeList;
use committee_runtime_api::CmRpcApi as CmStorageRuntimeApi;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

#[rpc]
pub trait CmRpcApi<BlockHash, AccountId>
where
    AccountId: Ord,
{
    #[rpc(name = "committee_getCommitteeList")]
    fn get_committee_list(&self, at: Option<BlockHash>) -> Result<CommitteeList<AccountId>>;
}

pub struct CmStorage<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> CmStorage<C, M> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block, AccountId> CmRpcApi<<Block as BlockT>::Hash, AccountId> for CmStorage<C, Block>
where
    Block: BlockT,
    AccountId: Clone + std::fmt::Display + Codec + Ord,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: CmStorageRuntimeApi<Block, AccountId>,
{
    fn get_committee_list(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<CommitteeList<AccountId>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_committee_list(&at);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
