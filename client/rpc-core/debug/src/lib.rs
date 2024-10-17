use dbc_client_evm_tracing::types::single;
use dbc_client_rpc_core_types::RequestBlockId;
use ethereum_types::H256;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};
use serde::Deserialize;

#[derive(Clone, Eq, PartialEq, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceParams {
    pub disable_storage: Option<bool>,
    pub disable_memory: Option<bool>,
    pub disable_stack: Option<bool>,
    /// Javascript tracer (we just check if it's Blockscout tracer string)
    pub tracer: Option<String>,
    pub timeout: Option<String>,
}

#[rpc(server)]
#[jsonrpsee::core::async_trait]
pub trait Debug {
    #[method(name = "debug_traceTransaction")]
    async fn trace_transaction(
        &self,
        transaction_hash: H256,
        params: Option<TraceParams>,
    ) -> RpcResult<single::TransactionTrace>;
    #[method(name = "debug_traceBlockByNumber", aliases = ["debug_traceBlockByHash"])]
    async fn trace_block(
        &self,
        id: RequestBlockId,
        params: Option<TraceParams>,
    ) -> RpcResult<Vec<single::TransactionTrace>>;
}
