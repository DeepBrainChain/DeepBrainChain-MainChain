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

use dbc_support::{rental_type::RentOrderDetail, rpc_types::RpcBalance, RentOrderId};
use rent_machine::MachineGPUOrder;
use rent_machine_runtime_api::RmRpcApi as RmStorageRuntimeApi;

#[rpc]
pub trait RmRpcApi<BlockHash, AccountId, BlockNumber, Balance>
where
    Balance: Display + FromStr,
{
    #[rpc(name = "rentMachine_getRentOrder")]
    fn get_rent_order(
        &self,
        rent_id: RentOrderId,
        at: Option<BlockHash>,
    ) -> Result<RentOrderDetail<AccountId, BlockNumber, RpcBalance<Balance>>>;

    #[rpc(name = "rentMachine_getRentList")]
    fn get_rent_list(&self, renter: AccountId, at: Option<BlockHash>) -> Result<Vec<RentOrderId>>;

    #[rpc(name = "rentMachine_isMachineRenter")]
    fn is_machine_renter(
        &self,
        machine_id: String,
        renter: AccountId,
        at: Option<BlockHash>,
    ) -> Result<bool>;

    #[rpc(name = "rentMachine_getMachineRentId")]
    fn get_machine_rent_id(
        &self,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> Result<MachineGPUOrder>;
}

pub struct RmStorage<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> RmStorage<C, M> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block, AccountId, BlockNumber, Balance>
    RmRpcApi<<Block as BlockT>::Hash, AccountId, BlockNumber, Balance> for RmStorage<C, Block>
where
    Block: BlockT,
    AccountId: Clone + std::fmt::Display + Codec + Ord,
    Balance: Codec + MaybeDisplay + Copy + FromStr,
    BlockNumber: Clone + std::fmt::Display + Codec,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: RmStorageRuntimeApi<Block, AccountId, BlockNumber, Balance>,
{
    fn get_rent_order(
        &self,
        rent_id: RentOrderId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<RentOrderDetail<AccountId, BlockNumber, RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result =
            api.get_rent_order(&at, rent_id).map(|order_detail| RentOrderDetail {
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
    ) -> Result<MachineGPUOrder> {
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
