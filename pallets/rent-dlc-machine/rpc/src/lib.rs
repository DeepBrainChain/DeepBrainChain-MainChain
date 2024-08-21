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
    RentOrderId,
};
pub use rent_dlc_machine_runtime_api::DlcRmRpcApi as RmDlcStorageRuntimeApi;

#[rpc(client, server)]
pub trait DlcRmRpcApi<BlockHash, AccountId, BlockNumber, Balance>
where
    Balance: Display + FromStr,
{
    #[method(name = "rentDlcMachine_getDlcRentOrder")]
    fn get_dlc_rent_order(
        &self,
        rent_id: RentOrderId,
        at: Option<BlockHash>,
    ) -> RpcResult<RentOrderDetail<AccountId, BlockNumber, RpcBalance<Balance>>>;

    #[method(name = "rentDlcMachine_getDlcRentList")]
    fn get_dlc_rent_list(
        &self,
        renter: AccountId,
        at: Option<BlockHash>,
    ) -> RpcResult<Vec<RentOrderId>>;

    #[method(name = "rentDlcMachine_isDlcMachineRenter")]
    fn is_dlc_machine_renter(
        &self,
        machine_id: String,
        renter: AccountId,
        at: Option<BlockHash>,
    ) -> RpcResult<bool>;

    #[method(name = "rentDlcMachine_getDlcMachineRentId")]
    fn get_dlc_machine_rent_id(
        &self,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> RpcResult<MachineGPUOrder>;
}

pub struct DlcRmStorage<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> DlcRmStorage<C, M> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block, AccountId, BlockNumber, Balance>
    DlcRmRpcApiServer<<Block as BlockT>::Hash, AccountId, BlockNumber, Balance>
    for DlcRmStorage<C, Block>
where
    Block: BlockT,
    AccountId: Clone + std::fmt::Display + Codec + Ord,
    Balance: Codec + MaybeDisplay + Copy + FromStr,
    BlockNumber: Clone + std::fmt::Display + Codec,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: RmDlcStorageRuntimeApi<Block, AccountId, BlockNumber, Balance>,
{
    fn get_dlc_rent_order(
        &self,
        rent_id: RentOrderId,
        at: Option<Block::Hash>,
    ) -> RpcResult<RentOrderDetail<AccountId, BlockNumber, RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api.get_dlc_rent_order(at_hash, rent_id);
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

    fn get_dlc_rent_list(
        &self,
        renter: AccountId,
        at: Option<Block::Hash>,
    ) -> RpcResult<Vec<RentOrderId>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let runtime_api_result = api.get_dlc_rent_list(at_hash, renter).map_err(|e| {
            JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Something wrong",
                Some(e.to_string()),
            )))
        })?;

        Ok(runtime_api_result)
    }

    fn is_dlc_machine_renter(
        &self,
        machine_id: String,
        renter: AccountId,
        at: Option<Block::Hash>,
    ) -> RpcResult<bool> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let machine_id = machine_id.as_bytes().to_vec();
        let runtime_api_result =
            api.is_dlc_machine_renter(at_hash, machine_id, renter).map_err(|e| {
                JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                    ErrorCode::InternalError.code(),
                    "Something wrong",
                    Some(e.to_string()),
                )))
            })?;
        Ok(runtime_api_result)
    }

    fn get_dlc_machine_rent_id(
        &self,
        machine_id: String,
        at: Option<Block::Hash>,
    ) -> RpcResult<MachineGPUOrder> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);

        let machine_id = machine_id.as_bytes().to_vec();
        let runtime_api_result = api.get_dlc_machine_rent_id(at_hash, machine_id).map_err(|e| {
            JsonRpseeError::Call(CallError::Custom(ErrorObject::owned(
                ErrorCode::InternalError.code(),
                "Something wrong",
                Some(e.to_string()),
            )))
        })?;

        Ok(runtime_api_result)
    }
}
