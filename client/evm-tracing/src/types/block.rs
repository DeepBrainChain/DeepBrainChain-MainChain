//! Types for tracing all Ethereum transactions of a block.

use super::serialization::*;
use serde::Serialize;

use ethereum_types::{H160, H256, U256};
use parity_scale_codec::{Decode, Encode};
use sp_std::vec::Vec;

#[derive(Clone, Eq, PartialEq, Debug, Encode, Decode, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionTrace {
    #[serde(flatten)]
    pub action: TransactionTraceAction,
    #[serde(serialize_with = "h256_0x_serialize")]
    pub block_hash: H256,
    pub block_number: u32,
    #[serde(flatten)]
    pub output: TransactionTraceOutput,
    pub subtraces: u32,
    pub trace_address: Vec<u32>,
    #[serde(serialize_with = "h256_0x_serialize")]
    pub transaction_hash: H256,
    pub transaction_position: u32,
}

#[derive(Clone, Eq, PartialEq, Debug, Encode, Decode, Serialize)]
#[serde(rename_all = "camelCase", tag = "type", content = "action")]
pub enum TransactionTraceAction {
    #[serde(rename_all = "camelCase")]
    Call {
        call_type: super::CallType,
        from: H160,
        gas: U256,
        #[serde(serialize_with = "bytes_0x_serialize")]
        input: Vec<u8>,
        to: H160,
        value: U256,
    },
    #[serde(rename_all = "camelCase")]
    Create {
        creation_method: super::CreateType,
        from: H160,
        gas: U256,
        #[serde(serialize_with = "bytes_0x_serialize")]
        init: Vec<u8>,
        value: U256,
    },
    #[serde(rename_all = "camelCase")]
    Suicide { address: H160, balance: U256, refund_address: H160 },
}

#[derive(Clone, Eq, PartialEq, Debug, Encode, Decode, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TransactionTraceOutput {
    Result(TransactionTraceResult),
    Error(#[serde(serialize_with = "string_serialize")] Vec<u8>),
}

#[derive(Clone, Eq, PartialEq, Debug, Encode, Decode, Serialize)]
#[serde(rename_all = "camelCase", untagged)]
pub enum TransactionTraceResult {
    #[serde(rename_all = "camelCase")]
    Call {
        gas_used: U256,
        #[serde(serialize_with = "bytes_0x_serialize")]
        output: Vec<u8>,
    },
    #[serde(rename_all = "camelCase")]
    Create {
        address: H160,
        #[serde(serialize_with = "bytes_0x_serialize")]
        code: Vec<u8>,
        gas_used: U256,
    },
    Suicide,
}
