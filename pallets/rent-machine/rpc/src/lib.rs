use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use rent_machine::{MachineId, RpcRentOrderDetail};
use rent_machine_runtime_api::RmRpcApi as RmStorageRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_rpc::number::NumberOrHex;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, MaybeDisplay},
};
use std::{convert::TryInto, sync::Arc};

#[rpc]
pub trait RmRpcApi<BlockHash, AccountId, ResponseType1, ResponseType2> {
    #[rpc(name = "rentMachine_getSum")]
    fn get_sum(&self, at: Option<BlockHash>) -> Result<u64>;

    #[rpc(name = "rentMachine_getRentOrder")]
    fn get_rent_order(
        &self,
        renter: AccountId,
        machine_id: MachineId,
        at: Option<BlockHash>,
    ) -> Result<ResponseType1>;

    #[rpc(name = "rentMachine_getRentList")]
    fn get_rent_list(&self, renter: AccountId, at: Option<BlockHash>) -> Result<ResponseType2>;
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
    RmRpcApi<
        <Block as BlockT>::Hash,
        AccountId,
        RpcRentOrderDetail<AccountId, BlockNumber, Balance>,
        Vec<MachineId>,
    > for RmStorage<C, Block>
where
    Block: BlockT,
    AccountId: Clone + std::fmt::Display + Codec + Ord,
    Balance: Codec + MaybeDisplay + Copy + TryInto<NumberOrHex>,
    BlockNumber: Clone + std::fmt::Display + Codec,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: RmStorageRuntimeApi<Block, AccountId, BlockNumber, Balance>,
{
    fn get_sum(&self, at: Option<<Block as BlockT>::Hash>) -> Result<u64> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_sum(&at);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_rent_order(
        &self,
        renter: AccountId,
        machine_id: MachineId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<RpcRentOrderDetail<AccountId, BlockNumber, Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let runtime_api_result = api.get_rent_order(&at, renter, machine_id);

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
    ) -> Result<Vec<MachineId>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_rent_list(&at, renter);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
