use codec::Codec;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use lease_committee::{
    LCCommitteeMachineList, LCMachineCommitteeList, MachineId, RpcLCCommitteeOps,
};
use lease_committee_runtime_api::LcRpcApi as LcStorageRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_rpc::number::NumberOrHex;
use sp_runtime::{
    generic::BlockId,
    traits::{Block as BlockT, MaybeDisplay},
};
use std::{convert::TryInto, sync::Arc};

#[rpc]
pub trait LcRpcApi<BlockHash, AccountId, ResponseType1, ResponseType2, ResponseType3> {
    #[rpc(name = "leaseCommittee_getSum")]
    fn get_sum(&self, at: Option<BlockHash>) -> Result<u64>;

    #[rpc(name = "leaseCommittee_getCommitteeMachineList")]
    fn get_committee_machine_list(
        &self,
        committee: AccountId,
        at: Option<BlockHash>,
    ) -> Result<ResponseType1>;

    #[rpc(name = "leaseCommittee_getCommitteeOps")]
    fn get_committee_ops(
        &self,
        committee: AccountId,
        machine_id: MachineId,
        at: Option<BlockHash>,
    ) -> Result<ResponseType2>;

    #[rpc(name = "leaseCommittee_getMachineCommitteeList")]
    fn get_machine_committee_list(
        &self,
        machine_id: MachineId,
        at: Option<BlockHash>,
    ) -> Result<ResponseType3>;
}

pub struct LcStorage<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> LcStorage<C, M> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block, AccountId, BlockNumber, Balance>
    LcRpcApi<
        <Block as BlockT>::Hash,
        AccountId,
        LCCommitteeMachineList,
        RpcLCCommitteeOps<BlockNumber, Balance>,
        LCMachineCommitteeList<AccountId, BlockNumber>,
    > for LcStorage<C, Block>
where
    Block: BlockT,
    AccountId: Clone + std::fmt::Display + Codec + Ord,
    Balance: Codec + MaybeDisplay + Copy + TryInto<NumberOrHex>,
    BlockNumber: Clone + std::fmt::Display + Codec,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: LcStorageRuntimeApi<Block, AccountId, BlockNumber, Balance>,
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

    fn get_committee_machine_list(
        &self,
        committee: AccountId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<LCCommitteeMachineList> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_committee_machine_list(&at, committee);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_committee_ops(
        &self,
        committee: AccountId,
        machine_id: MachineId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<RpcLCCommitteeOps<BlockNumber, Balance>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_committee_ops(&at, committee, machine_id);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }

    fn get_machine_committee_list(
        &self,
        machine_id: MachineId,
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<LCMachineCommitteeList<AccountId, BlockNumber>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));

        let runtime_api_result = api.get_machine_committee_list(&at, machine_id);
        runtime_api_result.map_err(|e| RpcError {
            code: ErrorCode::ServerError(9876),
            message: "Something wrong".into(),
            data: Some(format!("{:?}", e).into()),
        })
    }
}
