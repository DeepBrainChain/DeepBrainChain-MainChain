//! RPC interface for the transaction payment module.
#![warn(unused_crate_dependencies)]

use codec::Codec;

use jsonrpsee::{
    core::{Error as JsonRpseeError, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorCode, ErrorObject},
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, MaybeDisplay},
};
use std::{fmt::Display, str::FromStr, sync::Arc};

use dbc_support::{
    machine_type::{Latitude, Longitude},
    rpc_types::RpcBalance,
    EraIndex,
};
use online_profile::{
    rpc_types::{RpcLiveMachine, RpcMachineInfo, RpcStakerInfo, RpcStashMachine},
    PosInfo, SysInfoDetail,
};
pub use online_profile_runtime_api::OpRpcApi as OpStorageRuntimeApi;
#[rpc(client, server)]
pub trait OpRpcApi<BlockHash, AccountId, Balance, BlockNumber>
where
    Balance: Display + FromStr,
    AccountId: Ord,
{
    #[method(name = "onlineProfile_getStakerNum")]
    fn get_total_staker_num(&self, at: Option<BlockHash>) -> RpcResult<u64>;

    #[method(name = "onlineProfile_getOpInfo")]
    fn get_op_info(&self, at: Option<BlockHash>) -> RpcResult<SysInfoDetail<RpcBalance<Balance>>>;

    #[method(name = "onlineProfile_getStakerInfo")]
    fn get_staker_info(
        &self,
        account: AccountId,
        at: Option<BlockHash>,
    ) -> RpcResult<RpcStakerInfo<RpcBalance<Balance>, BlockNumber, AccountId>>;

    #[method(name = "onlineProfile_getMachineList")]
    fn get_machine_list(&self, at: Option<BlockHash>) -> RpcResult<RpcLiveMachine>;

    #[method(name = "onlineProfile_getMachineInfo")]
    fn get_machine_info(
        &self,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> RpcResult<RpcMachineInfo<AccountId, BlockNumber, RpcBalance<Balance>>>;

    #[method(name = "onlineProfile_getPosGpuInfo")]
    fn get_pos_gpu_info(
        &self,
        at: Option<BlockHash>,
    ) -> RpcResult<Vec<(Longitude, Latitude, PosInfo)>>;

    #[method(name = "onlineProfile_getMachineEraReward")]
    fn get_machine_era_reward(
        &self,
        machine_id: String,
        era_index: EraIndex,
        at: Option<BlockHash>,
    ) -> RpcResult<RpcBalance<Balance>>;

    #[method(name = "onlineProfile_getMachineEraReleasedReward")]
    fn get_machine_era_released_reward(
        &self,
        machine_id: String,
        era_index: EraIndex,
        at: Option<BlockHash>,
    ) -> RpcResult<RpcBalance<Balance>>;

    #[method(name = "onlineProfile_getStashEraReward")]
    fn get_stash_era_reward(
        &self,
        stash: AccountId,
        era_index: EraIndex,
        at: Option<BlockHash>,
    ) -> RpcResult<RpcBalance<Balance>>;

    #[method(name = "onlineProfile_getStashEraReleasedReward")]
    fn get_stash_era_released_reward(
        &self,
        stash: AccountId,
        era_index: EraIndex,
        at: Option<BlockHash>,
    ) -> RpcResult<RpcBalance<Balance>>;
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
    OpRpcApiServer<<Block as BlockT>::Hash, AccountId, Balance, BlockNumber> for OpStorage<C, Block>
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
    fn get_total_staker_num(&self, at: Option<<Block as BlockT>::Hash>) -> RpcResult<u64> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_total_staker_num(&at).map_err(|e| {
            JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Something wrong",
                Some(e.to_string()),
            )))
        })?;
        Ok(runtime_api_result)
    }

    fn get_op_info(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<SysInfoDetail<RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api
            .get_op_info(&at)
            .map(|sys_info| SysInfoDetail {
                total_gpu_num: sys_info.total_gpu_num,
                total_rented_gpu: sys_info.total_rented_gpu,
                total_staker: sys_info.total_staker,
                total_calc_points: sys_info.total_calc_points,
                total_stake: sys_info.total_stake.into(),
                total_rent_fee: sys_info.total_rent_fee.into(),
                total_burn_fee: sys_info.total_burn_fee.into(),
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

    fn get_staker_info(
        &self,
        account: AccountId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<RpcStakerInfo<RpcBalance<Balance>, BlockNumber, AccountId>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api
            .get_staker_info(&at, account)
            .map(|staker_info| {
                let tmp_info: RpcStashMachine<Balance> = staker_info.stash_statistic.clone().into();
                RpcStakerInfo {
                    stash_statistic: RpcStashMachine {
                        total_machine: tmp_info.total_machine,
                        online_machine: tmp_info.online_machine,
                        total_calc_points: tmp_info.total_calc_points,
                        total_gpu_num: tmp_info.total_gpu_num,
                        total_rented_gpu: tmp_info.total_rented_gpu,

                        total_earned_reward: staker_info.stash_statistic.total_earned_reward.into(),
                        total_claimed_reward: staker_info
                            .stash_statistic
                            .total_claimed_reward
                            .into(),
                        can_claim_reward: staker_info.stash_statistic.can_claim_reward.into(),
                        total_rent_fee: staker_info.stash_statistic.total_rent_fee.into(),
                        total_burn_fee: staker_info.stash_statistic.total_burn_fee.into(),
                    },
                    bonded_machines: staker_info.bonded_machines,
                }
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

    fn get_machine_list(&self, at: Option<<Block as BlockT>::Hash>) -> RpcResult<RpcLiveMachine> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_machine_list(&at).map_err(|e| {
            JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Something wrong",
                Some(e.to_string()),
            )))
        })?;

        Ok(runtime_api_result.into())
    }

    fn get_machine_info(
        &self,
        machine_id: String,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<RpcMachineInfo<AccountId, BlockNumber, RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let machine_id = machine_id.as_bytes().to_vec();

        let runtime_api_result = api.get_machine_info(&at, machine_id);
        if let Ok(Some(machine_info)) = runtime_api_result {
            return Ok(RpcMachineInfo {
                controller: machine_info.controller,
                machine_stash: machine_info.machine_stash,
                renters: machine_info.renters,
                last_machine_restake: machine_info.last_machine_restake,
                bonding_height: machine_info.bonding_height,
                online_height: machine_info.online_height,
                last_online_height: machine_info.last_online_height,
                init_stake_per_gpu: machine_info.init_stake_per_gpu.into(),
                stake_amount: machine_info.stake_amount.into(),
                machine_status: machine_info.machine_status,
                total_rented_duration: machine_info.total_rented_duration,
                total_rented_times: machine_info.total_rented_times,
                total_rent_fee: machine_info.total_rent_fee.into(),
                total_burn_fee: machine_info.total_burn_fee.into(),
                machine_info_detail: machine_info.machine_info_detail.into(),
                reward_committee: machine_info.reward_committee,
                reward_deadline: machine_info.reward_deadline,
            })
        };
        return Err(JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
            ErrorCode::InternalError.code(),
            "Something wrong",
            Some("NotFound"),
        ))))
    }

    fn get_pos_gpu_info(
        &self,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<Vec<(Longitude, Latitude, PosInfo)>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_pos_gpu_info(&at).map_err(|e| {
            JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Something wrong",
                Some(e.to_string()),
            )))
        })?;
        Ok(runtime_api_result)
    }

    fn get_machine_era_reward(
        &self,
        machine_id: String,
        era_index: EraIndex,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<RpcBalance<Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let machine_id = machine_id.as_bytes().to_vec();

        let runtime_api_result = api
            .get_machine_era_reward(&at, machine_id, era_index)
            .map(|balance| balance.into())
            .map_err(|e| {
                JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                    ErrorCode::InternalError.code(),
                    "Something wrong",
                    Some(e.to_string()),
                )))
            })?;
        Ok(runtime_api_result)
    }

    fn get_machine_era_released_reward(
        &self,
        machine_id: String,
        era_index: EraIndex,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<RpcBalance<Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let machine_id = machine_id.as_bytes().to_vec();

        let runtime_api_result = api
            .get_machine_era_released_reward(&at, machine_id, era_index)
            .map(|balance| balance.into())
            .map_err(|e| {
                JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                    ErrorCode::InternalError.code(),
                    "Something wrong",
                    Some(e.to_string()),
                )))
            })?;
        Ok(runtime_api_result)
    }

    fn get_stash_era_reward(
        &self,
        stash: AccountId,
        era_index: EraIndex,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<RpcBalance<Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api
            .get_stash_era_reward(&at, stash, era_index)
            .map(|balance| balance.into())
            .map_err(|e| {
                JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                    ErrorCode::InternalError.code(),
                    "Something wrong",
                    Some(e.to_string()),
                )))
            })?;
        Ok(runtime_api_result)
    }

    fn get_stash_era_released_reward(
        &self,
        stash: AccountId,
        era_index: EraIndex,
        at: Option<<Block as BlockT>::Hash>,
    ) -> RpcResult<RpcBalance<Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api
            .get_stash_era_released_reward(&at, stash, era_index)
            .map(|balance| balance.into())
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
