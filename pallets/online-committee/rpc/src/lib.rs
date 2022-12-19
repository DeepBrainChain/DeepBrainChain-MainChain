use codec::Codec;
use dbc_support::rpc_types::RpcBalance;
use jsonrpc_core::{Error as RpcError, ErrorCode, Result};
use jsonrpc_derive::rpc;
use online_committee::{
    rpc::RpcOCCommitteeOps, rpc_types::RpcOCCommitteeMachineList, OCMachineCommitteeList,
};

use online_committee_runtime_api::OcRpcApi as OcStorageRuntimeApi;
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};
use std::{fmt::Display, str::FromStr, sync::Arc};

#[rpc]
pub trait OcRpcApi<BlockHash, AccountId, BlockNumber, Balance>
where
    Balance: Display + FromStr,
{
    #[rpc(name = "onilneCommittee_getCommitteeMachineList")]
    fn get_committee_machine_list(
        &self,
        committee: AccountId,
        at: Option<BlockHash>,
    ) -> Result<RpcOCCommitteeMachineList>;

    #[rpc(name = "onlineCommittee_getCommitteeOps")]
    fn get_committee_ops(
        &self,
        committee: AccountId,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> Result<RpcOCCommitteeOps<BlockNumber, RpcBalance<Balance>>>;

    #[rpc(name = "onlineCommittee_getMachineCommitteeList")]
    fn get_machine_committee_list(
        &self,
        machine_id: String,
        at: Option<BlockHash>,
    ) -> Result<OCMachineCommitteeList<AccountId, BlockNumber>>;
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
    OcRpcApi<<Block as BlockT>::Hash, AccountId, BlockNumber, Balance> for OcStorage<C, Block>
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
        at: Option<<Block as BlockT>::Hash>,
    ) -> Result<RpcOCCommitteeMachineList> {
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
    ) -> Result<RpcOCCommitteeOps<BlockNumber, RpcBalance<Balance>>> {
        let api = self.client.runtime_api();
        let at = BlockId::hash(at.unwrap_or_else(|| self.client.info().best_hash));
        let machine_id = machine_id.as_bytes().to_vec();

        let runtime_api_result =
            api.get_committee_ops(&at, committee, machine_id).map(|ops| RpcOCCommitteeOps {
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
    ) -> Result<OCMachineCommitteeList<AccountId, BlockNumber>> {
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
}
