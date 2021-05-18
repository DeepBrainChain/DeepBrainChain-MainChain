//! RPC interface for the transaction payment module.

use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use online_profile::{StakerInfo, SysInfo};
use online_profile_runtime_api::SumStorageApi as SumStorageRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_rpc::number::NumberOrHex;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, MaybeDisplay},
};
use std::{convert::TryInto, sync::Arc};

#[rpc]
pub trait SumStorageApi<BlockHash, AccountId, ResponseType1, ResponseType2> {
    #[rpc(name = "onlineProfile_getSum")]
    fn get_sum(&self, at: Option<BlockHash>) -> Result<u32>;

    #[rpc(name = "onlineProfile_getOpInfo")]
    fn get_op_info(&self, at: Option<BlockHash>) -> Result<ResponseType1>;

    #[rpc(name = "onlineProfile_getStakerInfo")]
    fn get_staker_info(&self, at: Option<BlockHash>, account: AccountId) -> Result<ResponseType2>;
}

pub struct SumStorage<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> SumStorage<C, M> {
    pub fn new(client: Arc<C>) -> Self {
        Self {
            client,
            _marker: Default::default(),
        }
    }
}

impl<C, Block, AccountId, Balance>
    SumStorageApi<<Block as BlockT>::Hash, AccountId, SysInfo<Balance>, StakerInfo<Balance>>
    for SumStorage<C, Block>
where
    Block: BlockT,
    AccountId: Clone + std::fmt::Display + Codec,
    Balance: Codec + MaybeDisplay + Copy + TryInto<NumberOrHex>,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: SumStorageRuntimeApi<Block, AccountId, Balance>,
{
    fn get_sum(&self, at: Option<<Block as BlockT>::Hash>) -> Result<u32> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_sum(&at);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_op_info(&self, at: Option<<Block as BlockT>::Hash>) -> Result<SysInfo<Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_op_info(&at);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_staker_info(
        &self,
        at: Option<<Block as BlockT>::Hash>,
        account: AccountId,
    ) -> Result<StakerInfo<Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_staker_info(&at, account);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
