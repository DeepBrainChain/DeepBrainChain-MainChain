use codec::Codec;
use generic_func::{MachineId, RpcBalance};
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use rent_machine::RpcRentOrderDetail;
use rent_machine_runtime_api::RmRpcApi as RmStorageRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, MaybeDisplay},
};
use std::{fmt::Display, str::FromStr, sync::Arc};

#[rpc]
pub trait RmRpcApi<BlockHash, AccountId, BlockNumber, Balance>
where
    Balance: Display + FromStr,
{
    #[rpc(name = "rentMachine_getRentOrder")]
    fn get_rent_order(
        &self,
        renter: AccountId,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> Result<RpcRentOrderDetail<AccountId, BlockNumber, RpcBalance<Balance>>>;

    #[rpc(name = "rentMachine_getRentList")]
    fn get_rent_list(&self, renter: AccountId, at: Option<BlockHash>) -> Result<Vec<MachineId>>;

    #[rpc(name = "rentMachine_getMachineRenter")]
    fn get_machine_renter(&self, machine_id: MachineId, at: Option<BlockHash>) -> Result<Option<AccountId>>;
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

impl<C, Block, AccountId, BlockNumber, Balance> RmRpcApi<<Block as BlockT>::Hash, AccountId, BlockNumber, Balance>
    for RmStorage<C, Block>
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
        renter: AccountId,
        machine_id: String,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<RpcRentOrderDetail<AccountId, BlockNumber, RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let machine_id = machine_id.as_bytes().to_vec();

        let runtime_api_result = api.get_rent_order(&at, renter, machine_id).map(|order_detail| RpcRentOrderDetail {
            renter: order_detail.renter,
            rent_start: order_detail.rent_start,
            confirm_rent: order_detail.confirm_rent,
            rent_end: order_detail.rent_end,
            stake_amount: order_detail.stake_amount.into(),
        });
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_rent_list(&self, renter: AccountId, at: Option<<Block as BlockT>::Hash>) -> Result<Vec<MachineId>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_rent_list(&at, renter);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_machine_renter(
        &self,
        machine_id: MachineId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<Option<AccountId>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_machine_renter(&at, machine_id);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
