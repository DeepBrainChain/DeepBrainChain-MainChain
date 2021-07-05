use codec::Codec;
use generic_func::RpcBalance;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use simple_rpc::StakerListInfo;
use simple_rpc_runtime_api::SimpleRpcApi as SrStorageRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, MaybeDisplay},
};
use std::sync::Arc;
use std::{fmt::Display, str::FromStr};

#[rpc]
pub trait SimpleRpcApi<BlockHash, AccountId, Balance>
where
    Balance: Display + FromStr,
{
    #[rpc(name = "onlineProfile_getStakerIdentity")]
    fn get_staker_identity(&self, account: AccountId, at: Option<BlockHash>) -> Result<Vec<u8>>;

    #[rpc(name = "onlineProfile_getStakerListInfo")]
    fn get_staker_list_info(
        &self,
        cur_page: u64,
        per_page: u64,
        at: Option<BlockHash>,
    ) -> Result<Vec<StakerListInfo<RpcBalance<Balance>, AccountId>>>;
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

impl<C, Block, AccountId, Balance> SimpleRpcApi<<Block as BlockT>::Hash, AccountId, Balance>
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
    ) -> Result<Vec<StakerListInfo<RpcBalance<Balance>, AccountId>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result =
            api.get_staker_list_info(&at, cur_page, per_page).map(|staker_info_list| {
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
                    })
                }
                .collect::<Vec<_>>()
            });

        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
