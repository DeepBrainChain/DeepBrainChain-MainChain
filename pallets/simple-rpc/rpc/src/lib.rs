#![warn(unused_crate_dependencies)]

use dbc_support::rpc_types::RpcBalance;
use jsonrpsee::{
    core::{Error as JsonRpseeError, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorCode, ErrorObject},
};
use parity_scale_codec::Codec;
use simple_rpc::StakerListInfo;
pub use simple_rpc_runtime_api::SimpleRpcApi as SrStorageRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::{Block as BlockT, MaybeDisplay};
use std::{fmt::Display, str::FromStr, sync::Arc};

#[rpc(client, server)]
pub trait SimpleRpcApi<BlockHash, AccountId, Balance>
where
    Balance: Display + FromStr,
{
    #[method(name = "onlineProfile_getStakerIdentity")]
    fn get_staker_identity(&self, account: AccountId, at: Option<BlockHash>) -> RpcResult<String>;

    #[method(name = "onlineProfile_getStakerListInfo")]
    fn get_staker_list_info(
        &self,
        cur_page: u64,
        per_page: u64,
        at: Option<BlockHash>,
    ) -> RpcResult<Vec<StakerListInfo<RpcBalance<Balance>, AccountId>>>;
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

impl<C, Block, AccountId, Balance> SimpleRpcApiServer<<Block as BlockT>::Hash, AccountId, Balance>
    for SrStorage<C, Block>
where
    Block: BlockT,
    AccountId: Codec,
    Balance: Codec + MaybeDisplay + Copy + FromStr,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: SrStorageRuntimeApi<Block, AccountId, Balance>,
{
    fn get_staker_identity(
        &self,
        account: AccountId,
        at: Option<Block::Hash>,
    ) -> RpcResult<String> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api.get_staker_identity(at_hash, account).map_err(|e| {
            CallError::Custom(ErrorObject::owned(1, "Something wrong", Some(e.to_string())))
        })?;
        Ok(String::from_utf8_lossy(&runtime_api_result).to_string())
    }

    fn get_staker_list_info(
        &self,
        cur_page: u64,
        per_page: u64,
        at: Option<Block::Hash>,
    ) -> RpcResult<Vec<StakerListInfo<RpcBalance<Balance>, AccountId>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api
            .get_staker_list_info(at_hash, cur_page, per_page)
            .map(|staker_info_list| {
                {
                    staker_info_list.into_iter().map(|staker_info| StakerListInfo {
                        index: staker_info.index,
                        staker_name: staker_info.staker_name,
                        staker_account: staker_info.staker_account,
                        calc_points: staker_info.calc_points,
                        total_gpu_num: staker_info.total_gpu_num,
                        total_rented_gpu: staker_info.total_rented_gpu,
                        total_rent_fee: staker_info.total_rent_fee.into(),
                        total_burn_fee: staker_info.total_burn_fee.into(),
                        total_reward: staker_info.total_reward.into(),
                        total_released_reward: staker_info.total_released_reward.into(),
                    })
                }
                .collect::<Vec<_>>()
            })
            .map_err(|e| {
                JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                    ErrorCode::InternalError.code(),
                    "Something wrong",
                    Some(e.to_string()),
                )))
            })?;
        Ok(runtime_api_result)
    }
}
