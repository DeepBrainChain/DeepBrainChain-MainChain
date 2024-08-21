#![warn(unused_crate_dependencies)]

use jsonrpsee::{
    core::{Error as JsonRpseeError, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorCode, ErrorObject},
};
use parity_scale_codec::Codec;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::traits::{Block as BlockT, MaybeDisplay};
use std::{fmt::Display, str::FromStr, sync::Arc};

use dbc_support::{
    rental_type::{MachineGPUOrder, RentOrderDetail},
    rpc_types::RpcBalance,
    verify_online::OCMachineCommitteeList,
    RentOrderId,
};
use terminating_rental::rpc_types::{
    RpcIRCommitteeOps, RpcLiveMachine, RpcMachineInfo, RpcOCCommitteeMachineList, RpcStakerInfo,
    RpcStashMachine,
};

pub use terminating_rental_runtime_api::IrRpcApi as IrStorageRuntimeApi;

#[rpc(client, server)]
pub trait IrRpcApi<BlockHash, AccountId, Balance, BlockNumber>
where
    Balance: Display + FromStr,
    AccountId: Ord,
{
    #[method(name = "terminatingRental_getStakerNum")]
    fn get_total_staker_num(&self, at: Option<BlockHash>) -> RpcResult<u64>;

    #[method(name = "terminatingRental_getStakerInfo")]
    fn get_staker_info(
        &self,
        account: AccountId,
        at: Option<BlockHash>,
    ) -> RpcResult<RpcStakerInfo<RpcBalance<Balance>, BlockNumber, AccountId>>;

    #[method(name = "terminatingRental_getMachineList")]
    fn get_machine_list(&self, at: Option<BlockHash>) -> RpcResult<RpcLiveMachine>;

    #[method(name = "terminatingRental_getMachineInfo")]
    fn get_machine_info(
        &self,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> RpcResult<RpcMachineInfo<AccountId, BlockNumber, RpcBalance<Balance>>>;

    #[method(name = "terminatingRental_getCommitteeMachineList")]
    fn get_committee_machine_list(
        &self,
        committee: AccountId,
        at: Option<BlockHash>,
    ) -> RpcResult<RpcOCCommitteeMachineList>;

    #[method(name = "terminatingRental_getCommitteeOps")]
    fn get_committee_ops(
        &self,
        committee: AccountId,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> RpcResult<RpcIRCommitteeOps<BlockNumber, RpcBalance<Balance>>>;

    #[method(name = "terminatingRental_getMachineCommitteeList")]
    fn get_machine_committee_list(
        &self,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> RpcResult<OCMachineCommitteeList<AccountId, BlockNumber>>;

    #[method(name = "terminatingRental_getRentOrder")]
    fn get_rent_order(
        &self,
        rent_id: RentOrderId,
        at: Option<BlockHash>,
    ) -> RpcResult<RentOrderDetail<AccountId, BlockNumber, RpcBalance<Balance>>>;

    #[method(name = "terminatingRental_getRentList")]
    fn get_rent_list(
        &self,
        renter: AccountId,
        at: Option<BlockHash>,
    ) -> RpcResult<Vec<RentOrderId>>;

    #[method(name = "terminatingRental_isMachineRenter")]
    fn is_machine_renter(
        &self,
        machine_id: String,
        renter: AccountId,
        at: Option<BlockHash>,
    ) -> RpcResult<bool>;

    #[method(name = "terminatingRental_getMachineRentId")]
    fn get_machine_rent_id(
        &self,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> RpcResult<MachineGPUOrder>;
}

pub struct IrStorage<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> IrStorage<C, M> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block, AccountId, Balance, BlockNumber>
    IrRpcApiServer<<Block as BlockT>::Hash, AccountId, Balance, BlockNumber> for IrStorage<C, Block>
where
    Block: BlockT,
    AccountId: Clone + std::fmt::Display + Codec + Ord,
    Balance: Codec + MaybeDisplay + Copy + FromStr,
    BlockNumber: Clone + std::fmt::Display + Codec,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: IrStorageRuntimeApi<Block, AccountId, Balance, BlockNumber>,
{
    fn get_total_staker_num(&self, at: Option<Block::Hash>) -> RpcResult<u64> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api.get_total_staker_num(at_hash).map_err(|e| {
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
        at: Option<Block::Hash>,
    ) -> RpcResult<RpcStakerInfo<RpcBalance<Balance>, BlockNumber, AccountId>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api
            .get_staker_info(at_hash, account)
            .map(|staker_info| {
                let tmp_info: RpcStashMachine<Balance> = staker_info.stash_statistic.clone().into();
                RpcStakerInfo {
                    stash_statistic: RpcStashMachine {
                        total_machine: tmp_info.total_machine,
                        online_machine: tmp_info.online_machine,
                        total_calc_points: tmp_info.total_calc_points,
                        total_gpu_num: tmp_info.total_gpu_num,
                        total_rented_gpu: tmp_info.total_rented_gpu,

                        total_rent_fee: staker_info.stash_statistic.total_rent_fee.into(),
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

    fn get_machine_list(&self, at: Option<Block::Hash>) -> RpcResult<RpcLiveMachine> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api.get_machine_list(at_hash).map_err(|e| {
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
        at: Option<Block::Hash>,
    ) -> RpcResult<RpcMachineInfo<AccountId, BlockNumber, RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        let machine_id = machine_id.as_bytes().to_vec();

        let runtime_api_result = api.get_machine_info(at_hash, machine_id);
        if let Ok(Some(machine_info)) = runtime_api_result {
            return Ok(RpcMachineInfo {
                machine_stash: machine_info.machine_stash,
                renters: machine_info.renters,
                bonding_height: machine_info.bonding_height,
                online_height: machine_info.online_height,
                last_online_height: machine_info.last_online_height,
                stake_amount: machine_info.stake_amount.into(),
                machine_status: machine_info.machine_status,
                total_rented_duration: machine_info.total_rented_duration,
                total_rented_times: machine_info.total_rented_times,
                total_rent_fee: machine_info.total_rent_fee.into(),
                machine_info_detail: machine_info.machine_info_detail.into(),
                reward_committee: machine_info.reward_committee,
            });
        };
        return Err(JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
            ErrorCode::InternalError.code(),
            "Something wrong",
            Some("NotFound"),
        ))));
    }

    fn get_committee_machine_list(
        &self,
        committee: AccountId,
        at: Option<Block::Hash>,
    ) -> RpcResult<RpcOCCommitteeMachineList> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result =
            api.get_committee_machine_list(at_hash, committee).map_err(|e| {
                JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                    ErrorCode::InternalError.code(),
                    "Something wrong",
                    Some(e.to_string()),
                )))
            })?;

        Ok(runtime_api_result.into())
    }

    fn get_committee_ops(
        &self,
        committee: AccountId,
        machine_id: String,
        at: Option<Block::Hash>,
    ) -> RpcResult<RpcIRCommitteeOps<BlockNumber, RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        let machine_id = machine_id.as_bytes().to_vec();

        let runtime_api_result = api.get_committee_ops(at_hash, committee, machine_id);
        if let Ok(Some(ops)) = runtime_api_result {
            return Ok(RpcIRCommitteeOps {
                booked_time: ops.booked_time,
                staked_dbc: ops.staked_dbc.into(),
                verify_time: ops.verify_time,
                confirm_hash: ops.confirm_hash,
                hash_time: ops.hash_time,
                confirm_time: ops.confirm_time,
                machine_status: ops.machine_status,
                machine_info: ops.machine_info,
            });
        }
        return Err(JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
            ErrorCode::InternalError.code(),
            "Something wrong",
            Some("NotFound"),
        ))));
    }

    fn get_machine_committee_list(
        &self,
        machine_id: String,
        at: Option<Block::Hash>,
    ) -> RpcResult<OCMachineCommitteeList<AccountId, BlockNumber>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        let machine_id = machine_id.as_bytes().to_vec();

        let runtime_api_result =
            api.get_machine_committee_list(at_hash, machine_id).map_err(|e| {
                JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                    ErrorCode::InternalError.code(),
                    "Something wrong",
                    Some(e.to_string()),
                )))
            })?;
        Ok(runtime_api_result)
    }

    fn get_rent_order(
        &self,
        rent_id: RentOrderId,
        at: Option<Block::Hash>,
    ) -> RpcResult<RentOrderDetail<AccountId, BlockNumber, RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api.get_rent_order(at_hash, rent_id);
        if let Ok(Some(order_detail)) = runtime_api_result {
            return Ok(RentOrderDetail {
                machine_id: order_detail.machine_id,
                renter: order_detail.renter,
                rent_start: order_detail.rent_start,
                confirm_rent: order_detail.confirm_rent,
                rent_end: order_detail.rent_end,
                stake_amount: order_detail.stake_amount.into(),
                rent_status: order_detail.rent_status,
                gpu_num: order_detail.gpu_num,
                gpu_index: order_detail.gpu_index,
            });
        }
        return Err(JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
            ErrorCode::InternalError.code(),
            "Something wrong",
            Some("NotFound"),
        ))));
    }

    fn get_rent_list(
        &self,
        renter: AccountId,
        at: Option<Block::Hash>,
    ) -> RpcResult<Vec<RentOrderId>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api.get_rent_list(at_hash, renter).map_err(|e| {
            JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Something wrong",
                Some(e.to_string()),
            )))
        })?;
        Ok(runtime_api_result)
    }

    fn is_machine_renter(
        &self,
        machine_id: String,
        renter: AccountId,
        at: Option<Block::Hash>,
    ) -> RpcResult<bool> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let machine_id = machine_id.as_bytes().to_vec();
        let runtime_api_result =
            api.is_machine_renter(at_hash, machine_id, renter).map_err(|e| {
                JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                    ErrorCode::InternalError.code(),
                    "Something wrong",
                    Some(e.to_string()),
                )))
            })?;
        Ok(runtime_api_result)
    }

    fn get_machine_rent_id(
        &self,
        machine_id: String,
        at: Option<Block::Hash>,
    ) -> RpcResult<MachineGPUOrder> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let machine_id = machine_id.as_bytes().to_vec();
        let runtime_api_result = api.get_machine_rent_id(at_hash, machine_id).map_err(|e| {
            JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Something wrong",
                Some(e.to_string()),
            )))
        })?;
        Ok(runtime_api_result)
    }
}
