use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, MaybeDisplay},
};
use std::{fmt::Display, str::FromStr, sync::Arc};

use generic_func::RpcBalance;
use terminating_rental::{
    rpc_types::{
        RpcIRCommitteeMachineList, RpcIRCommitteeOps, RpcLiveMachine, RpcMachineInfo,
        RpcStakerInfo, RpcStashMachine,
    },
    IRMachineCommitteeList, IRMachineGPUOrder, IRRentOrderDetail, RentOrderId,
};
use terminating_rental_runtime_api::IrRpcApi as IrStorageRuntimeApi;

#[rpc]
pub trait IrRpcApi<BlockHash, AccountId, Balance, BlockNumber>
where
    Balance: Display + FromStr,
    AccountId: Ord,
{
    #[rpc(name = "terminatingRental_getStakerNum")]
    fn get_total_staker_num(&self, at: Option<BlockHash>) -> Result<u64>;

    #[rpc(name = "terminatingRental_getStakerInfo")]
    fn get_staker_info(
        &self,
        account: AccountId,
        at: Option<BlockHash>,
    ) -> Result<RpcStakerInfo<RpcBalance<Balance>, BlockNumber, AccountId>>;

    #[rpc(name = "terminatingRental_getMachineList")]
    fn get_machine_list(&self, at: Option<BlockHash>) -> Result<RpcLiveMachine>;

    #[rpc(name = "terminatingRental_getMachineInfo")]
    fn get_machine_info(
        &self,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> Result<RpcMachineInfo<AccountId, BlockNumber, RpcBalance<Balance>>>;

    #[rpc(name = "terminatingRental_getCommitteeMachineList")]
    fn get_committee_machine_list(
        &self,
        committee: AccountId,
        at: Option<BlockHash>,
    ) -> Result<RpcIRCommitteeMachineList>;

    #[rpc(name = "terminatingRental_getCommitteeOps")]
    fn get_committee_ops(
        &self,
        committee: AccountId,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> Result<RpcIRCommitteeOps<BlockNumber, RpcBalance<Balance>>>;

    #[rpc(name = "terminatingRental_getMachineCommitteeList")]
    fn get_machine_committee_list(
        &self,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> Result<IRMachineCommitteeList<AccountId, BlockNumber>>;

    #[rpc(name = "terminatingRental_getRentOrder")]
    fn get_rent_order(
        &self,
        rent_id: RentOrderId,
        at: Option<BlockHash>,
    ) -> Result<IRRentOrderDetail<AccountId, BlockNumber, RpcBalance<Balance>>>;

    #[rpc(name = "terminatingRental_getRentList")]
    fn get_rent_list(&self, renter: AccountId, at: Option<BlockHash>) -> Result<Vec<RentOrderId>>;

    #[rpc(name = "terminatingRental_isMachineRenter")]
    fn is_machine_renter(
        &self,
        machine_id: String,
        renter: AccountId,
        at: Option<BlockHash>,
    ) -> Result<bool>;

    #[rpc(name = "terminatingRental_getMachineRentId")]
    fn get_machine_rent_id(
        &self,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> Result<IRMachineGPUOrder>;
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
    IrRpcApi<<Block as BlockT>::Hash, AccountId, Balance, BlockNumber> for IrStorage<C, Block>
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

    fn get_staker_info(
        &self,
        account: AccountId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<RpcStakerInfo<RpcBalance<Balance>, BlockNumber, AccountId>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_staker_info(&at, account).map(|staker_info| {
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
        });

        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_machine_list(&self, at: Option<<Block as BlockT>::Hash>) -> Result<RpcLiveMachine> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_machine_list(&at);

        match runtime_api_result {
            Ok(data) => Ok(data.into()),
            Err(e) => Err(RpcError {
                code: ErrorCode::ServerError(9876),
                message: "Something wrong".into(),
                data: Some(format!("{:?}", e).into()),
            }),
        }
    }

    fn get_machine_info(
        &self,
        machine_id: String,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<RpcMachineInfo<AccountId, BlockNumber, RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let machine_id = machine_id.as_bytes().to_vec();

        let runtime_api_result =
            api.get_machine_info(&at, machine_id).map(|machine_info| RpcMachineInfo {
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
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_committee_machine_list(
        &self,
        committee: AccountId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<RpcIRCommitteeMachineList> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_committee_machine_list(&at, committee);

        match runtime_api_result {
            Ok(data) => Ok(data.into()),
            Err(e) => Err(RpcError {
                code: ErrorCode::ServerError(9876),
                message: "Something wrong".into(),
                data: Some(format!("{:?}", e).into()),
            }),
        }
    }

    fn get_committee_ops(
        &self,
        committee: AccountId,
        machine_id: String,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<RpcIRCommitteeOps<BlockNumber, RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let machine_id = machine_id.as_bytes().to_vec();

        let runtime_api_result =
            api.get_committee_ops(&at, committee, machine_id).map(|ops| RpcIRCommitteeOps {
                booked_time: ops.booked_time,
                staked_dbc: ops.staked_dbc.into(),
                verify_time: ops.verify_time,
                confirm_hash: ops.confirm_hash,
                hash_time: ops.hash_time,
                confirm_time: ops.confirm_time,
                machine_status: ops.machine_status,
                machine_info: ops.machine_info,
            });
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_machine_committee_list(
        &self,
        machine_id: String,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<IRMachineCommitteeList<AccountId, BlockNumber>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let machine_id = machine_id.as_bytes().to_vec();

        let runtime_api_result = api.get_machine_committee_list(&at, machine_id);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_rent_order(
        &self,
        rent_id: RentOrderId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<IRRentOrderDetail<AccountId, BlockNumber, RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result =
            api.get_rent_order(&at, rent_id).map(|order_detail| IRRentOrderDetail {
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
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_rent_list(
        &self,
        renter: AccountId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Vec<RentOrderId>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_rent_list(&at, renter);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn is_machine_renter(
        &self,
        machine_id: String,
        renter: AccountId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<bool> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let machine_id = machine_id.as_bytes().to_vec();
        let runtime_api_result = api.is_machine_renter(&at, machine_id, renter);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_machine_rent_id(
        &self,
        machine_id: String,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<IRMachineGPUOrder> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let machine_id = machine_id.as_bytes().to_vec();
        let runtime_api_result = api.get_machine_rent_id(&at, machine_id);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
