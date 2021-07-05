//! RPC interface for the transaction payment module.

use codec::Codec;
use generic_func::RpcBalance;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use online_profile::{LiveMachine, PosInfo, RPCMachineInfo, RpcSysInfo, StakerInfo};
use online_profile_runtime_api::OpRpcApi as OpStorageRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, MaybeDisplay},
};
use std::{fmt::Display, str::FromStr, sync::Arc};

#[rpc]
pub trait OpRpcApi<BlockHash, AccountId, Balance, BlockNumber>
where
    Balance: Display + FromStr,
{
    #[rpc(name = "onlineProfile_getStakerNum")]
    fn get_total_staker_num(&self, at: Option<BlockHash>) -> Result<u64>;

    #[rpc(name = "onlineProfile_getOpInfo")]
    fn get_op_info(&self, at: Option<BlockHash>) -> Result<RpcSysInfo<RpcBalance<Balance>>>;

    #[rpc(name = "onlineProfile_getStakerInfo")]
    fn get_staker_info(
        &self,
        account: AccountId,
        at: Option<BlockHash>,
    ) -> Result<StakerInfo<RpcBalance<Balance>>>;

    #[rpc(name = "onlineProfile_getMachineList")]
    fn get_machine_list(&self, at: Option<BlockHash>) -> Result<LiveMachine>;

    #[rpc(name = "onlineProfile_getMachineInfo")]
    fn get_machine_info(
        &self,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> Result<RPCMachineInfo<AccountId, BlockNumber, RpcBalance<Balance>>>;

    #[rpc(name = "onlineProfile_getPosGpuInfo")]
    fn get_pos_gpu_info(&self, at: Option<BlockHash>) -> Result<Vec<(i64, i64, PosInfo)>>;
}

pub struct OpStorage<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> OpStorage<C, M> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block, AccountId, Balance, BlockNumber>
    OpRpcApi<<Block as BlockT>::Hash, AccountId, Balance, BlockNumber> for OpStorage<C, Block>
where
    Block: BlockT,
    AccountId: Clone + std::fmt::Display + Codec + Ord,
    Balance: Codec + MaybeDisplay + Copy + FromStr,
    BlockNumber: Clone + std::fmt::Display + Codec,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: OpStorageRuntimeApi<Block, AccountId, Balance, BlockNumber>,
{
    fn get_total_staker_num(&self, at: Option<<Block as BlockT>::Hash>) -> Result<u64> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_total_staker_num(&at);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_op_info(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<RpcSysInfo<RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_op_info(&at).map(|sys_info| RpcSysInfo {
            total_gpu_num: sys_info.total_gpu_num,
            total_rented_gpu: sys_info.total_rented_gpu,
            total_staker: sys_info.total_staker,
            total_calc_points: sys_info.total_calc_points,
            total_stake: sys_info.total_stake.into(),
            total_rent_fee: sys_info.total_rent_fee.into(),
            total_burn_fee: sys_info.total_burn_fee.into(),
        });
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_staker_info(
        &self,
        account: AccountId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<StakerInfo<RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_staker_info(&at, account).map(|staker_info| StakerInfo {
            calc_points: staker_info.calc_points,
            gpu_num: staker_info.gpu_num,
            total_reward: staker_info.total_reward.into(),
        });
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_machine_list(&self, at: Option<<Block as BlockT>::Hash>) -> Result<LiveMachine> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_machine_list(&at);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_machine_info(
        &self,
        machine_id: String,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<RPCMachineInfo<AccountId, BlockNumber, RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let machine_id = machine_id.as_bytes().to_vec();

        let runtime_api_result =
            api.get_machine_info(&at, machine_id).map(|machine_info| RPCMachineInfo {
                machine_owner: machine_info.machine_owner,
                bonding_height: machine_info.bonding_height,
                stake_amount: machine_info.stake_amount.into(),
                machine_status: machine_info.machine_status,
                total_rented_duration: machine_info.total_rented_duration,
                total_rented_times: machine_info.total_rented_times,
                total_rent_fee: machine_info.total_rent_fee.into(),
                total_burn_fee: machine_info.total_burn_fee.into(),
                machine_info_detail: machine_info.machine_info_detail,
                reward_committee: machine_info.reward_committee,
                reward_deadline: machine_info.reward_deadline,
            });
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_pos_gpu_info(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Vec<(i64, i64, PosInfo)>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_pos_gpu_info(&at);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
