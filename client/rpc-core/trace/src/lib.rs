use dbc_client_evm_tracing::types::block::TransactionTrace;
use dbc_client_rpc_core_types::RequestBlockId;
use ethereum_types::H160;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use serde::Deserialize;

#[rpc(server)]
#[jsonrpsee::core::async_trait]
pub trait Trace {
    #[method(name = "trace_filter")]
    async fn filter(&self, filter: FilterRequest) -> RpcResult<Vec<TransactionTrace>>;
}

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterRequest {
    /// (optional?) From this block.
    pub from_block: Option<RequestBlockId>,

    /// (optional?) To this block.
    pub to_block: Option<RequestBlockId>,

    /// (optional) Sent from these addresses.
    pub from_address: Option<Vec<H160>>,

    /// (optional) Sent to these addresses.
    pub to_address: Option<Vec<H160>>,

    /// (optional) The offset trace number
    pub after: Option<u32>,

    /// (optional) Integer number of traces to display in a batch.
    pub count: Option<u32>,
}
