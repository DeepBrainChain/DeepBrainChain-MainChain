use codec::Codec;

use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use simple_rpc::StakerListInfo;
use simple_rpc_runtime_api::SimpleRpcApi as SrStorageRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::sync::Arc;

#[rpc]
pub trait SimpleRpcApi<BlockHash, AccountId, ResponseType1, ResponseType2> {
    #[rpc(name = "onlineProfile_getStakerIdentity")]
    fn get_staker_identity(
        &self,
        account: AccountId,
        at: Option<BlockHash>,
    ) -> Result<ResponseType1>;

    #[rpc(name = "onlineProfile_getStakerListInfo")]
    fn get_staker_list_info(
        &self,
        cur_page: u64,
        per_page: u64,
        at: Option<BlockHash>,
    ) -> Result<ResponseType2>;
}

pub struct SrStorage<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> SrStorage<C, M> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block, AccountId, Balance>
    SimpleRpcApi<
        <Block as BlockT>::Hash,
        AccountId,
        Vec<u8>,
        Vec<StakerListInfo<Balance, AccountId>>,
    > for SrStorage<C, Block>
where
    Block: BlockT,
    AccountId: Codec,
    Balance: Codec,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: SrStorageRuntimeApi<Block, AccountId, Balance>,
{
    fn get_staker_identity(
        &self,
        account: AccountId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Vec<u8>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_staker_identity(&at, account);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_staker_list_info(
        &self,
        cur_page: u64,
        per_page: u64,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Vec<StakerListInfo<Balance, AccountId>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_staker_list_info(&at, cur_page, per_page);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
