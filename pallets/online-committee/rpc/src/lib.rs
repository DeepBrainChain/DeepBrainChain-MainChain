#![warn(unused_crate_dependencies)]

use codec::Codec;
use jsonrpsee::{
    core::{Error as JsonRpseeError, RpcResult},
    proc_macros::rpc,
    types::error::{CallError, ErrorCode, ErrorObject},
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::{fmt::Display, str::FromStr, sync::Arc};

use dbc_support::{rpc_types::RpcBalance, verify_online::OCMachineCommitteeList};
use online_committee::{rpc::RpcOCCommitteeOps, rpc_types::RpcOCCommitteeMachineList};
pub use online_committee_runtime_api::OcRpcApi as OcStorageRuntimeApi;

#[rpc(client, server)]
pub trait OcRpcApi<BlockHash, AccountId, BlockNumber, Balance>
where
    Balance: Display + FromStr,
{
    #[method(name = "onlineCommittee_getCommitteeMachineList")]
    fn get_committee_machine_list(
        &self,
        committee: AccountId,
        at: Option<BlockHash>,
    ) -> RpcResult<RpcOCCommitteeMachineList>;

    #[method(name = "onlineCommittee_getCommitteeOps")]
    fn get_committee_ops(
        &self,
        committee: AccountId,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> RpcResult<RpcOCCommitteeOps<BlockNumber, RpcBalance<Balance>>>;

    #[method(name = "onlineCommittee_getMachineCommitteeList")]
    fn get_machine_committee_list(
        &self,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> RpcResult<OCMachineCommitteeList<AccountId, BlockNumber>>;
}

pub struct OcStorage<C, M> {
    client: Arc<C>,
    _marker: std::marker::PhantomData<M>,
}

impl<C, M> OcStorage<C, M> {
    pub fn new(client: Arc<C>) -> Self {
        Self { client, _marker: Default::default() }
    }
}

impl<C, Block, AccountId, BlockNumber, Balance>
    OcRpcApiServer<<Block as BlockT>::Hash, AccountId, BlockNumber, Balance> for OcStorage<C, Block>
where
    Block: BlockT,
    AccountId: Clone + std::fmt::Display + Codec + Ord,
    BlockNumber: Clone + std::fmt::Display + Codec,
    Balance: Codec + std::fmt::Display + FromStr,
    C: Send + Sync + 'static,
    C: ProvideRuntimeApi<Block>,
    C: HeaderBackend<Block>,
    C::Api: OcStorageRuntimeApi<Block, AccountId, BlockNumber, Balance>,
{
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
    ) -> RpcResult<RpcOCCommitteeOps<BlockNumber, RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at_hash = at.unwrap_or_else(|| self.client.info().best_hash);
        let machine_id = machine_id.as_bytes().to_vec();

        let runtime_api_result = api.get_committee_ops(at_hash, committee, machine_id);
        if let Ok(Some(ops)) = runtime_api_result {
            return Ok(RpcOCCommitteeOps {
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
}
