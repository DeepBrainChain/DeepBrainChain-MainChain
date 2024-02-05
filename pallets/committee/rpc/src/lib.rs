#![warn(unused_crate_dependencies)]

use codec::Codec;
use committee::CommitteeList;
pub use committee_runtime_api::CmRpcApi as CmStorageRuntimeApi;
use jsonrpsee::{
    core::{Error as JsonRpseeError, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorCode, ErrorObject},
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::Block as BlockT;
use std::sync::Arc;

#[rpc(client, server)]
pub trait CmRpcApi<BlockHash, AccountId>
where
    AccountId: Ord,
{
    #[method(name = "committee_getCommitteeList")]
    fn get_committee_list(&self, at: Option<BlockHash>) -> RpcResult<CommitteeList<AccountId>>;
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

impl<C, Block, AccountId> CmRpcApiServer<<Block as BlockT>::Hash, AccountId> for CmStorage<C, Block>
where
    Block: BlockT,
    AccountId: Clone + std::fmt::Display + Codec + Ord,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: CmStorageRuntimeApi<Block, AccountId>,
{
    fn get_committee_list(&self, at: Option<Block::Hash>) -> RpcResult<CommitteeList<AccountId>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api.get_committee_list(at_hash).map_err(|e| {
            JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Something wrong",
                Some(e.to_string()),
            )))
        })?;
        Ok(runtime_api_result)
    }
}
