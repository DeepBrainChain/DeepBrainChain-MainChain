use ethereum_types::U256;
use jsonrpsee::{core::RpcResult, proc_macros::rpc};

mod types;

pub use crate::types::{Get as GetT, Summary, Transaction, TransactionMap, TxPoolResult};

#[rpc(server)]
pub trait TxPool {
    #[method(name = "txpool_content")]
    fn content(&self) -> RpcResult<TxPoolResult<TransactionMap<Transaction>>>;

    #[method(name = "txpool_inspect")]
    fn inspect(&self) -> RpcResult<TxPoolResult<TransactionMap<Summary>>>;

    #[method(name = "txpool_status")]
    fn status(&self) -> RpcResult<TxPoolResult<U256>>;
}
