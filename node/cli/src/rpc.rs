//! A collection of node-specific RPC methods.
//! Substrate provides the `sc-rpc` crate, which defines the core RPC layer
//! used by Substrate nodes. This file extends those RPC definitions with
//! capabilities that are specific to this project's runtime configuration.

use jsonrpsee::RpcModule;
use std::sync::Arc;

use dbc_node_common::{cli_opt::EthApi as EthApiCmd, rpc::TracingConfig};
use dbc_runtime::{opaque::Block, AccountId, Balance, BlockNumber, Index};

use dbc_node_common::rpc::BabeDeps;
use sp_api::{CallApiAt, ProvideRuntimeApi};
use sp_block_builder::BlockBuilder;
use sp_blockchain::{
    Backend as BlockchainBackend, Error as BlockChainError, HeaderBackend, HeaderMetadata,
};
use sp_consensus::SelectChain;
use sp_runtime::traits::BlakeTwo256;

use dbc_node_common::rpc::{FullDeps, GrandpaDeps};
use sc_client_api::{
    backend::{Backend, StateBackend, StorageProvider},
    AuxStore, BlockchainEvents,
};
use sc_transaction_pool::ChainApi;
use sc_transaction_pool_api::TransactionPool;

/// Instantiate all full RPC extensions.
pub fn create_full<C, P, BE, SC, A>(
    deps: FullDeps<C, P, BE, SC, A>,
    maybe_tracing_config: Option<TracingConfig>,
    pubsub_notification_sinks: Arc<
        fc_mapping_sync::EthereumBlockNotificationSinks<
            fc_mapping_sync::EthereumBlockNotification<Block>,
        >,
    >,
) -> Result<RpcModule<()>, sc_service::Error>
where
    BE: Backend<Block> + Send + Sync + 'static,
    BE::State: StateBackend<BlakeTwo256>,
    BE::Blockchain: BlockchainBackend<Block>,
    C: ProvideRuntimeApi<Block>,
    C: BlockchainEvents<Block>,
    C: StorageProvider<Block, BE>,
    C: HeaderBackend<Block> + HeaderMetadata<Block, Error = BlockChainError> + 'static,
    C: CallApiAt<Block>,
    C: AuxStore,
    C: StorageProvider<Block, BE>,
    C: Send + Sync + 'static,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
    C::Api: BlockBuilder<Block>,
    C::Api: fp_rpc::EthereumRuntimeRPCApi<Block>,
    C::Api: fp_rpc::ConvertTransactionRuntimeApi<Block>,
    C::Api: dbc_primitives_rpc_txpool::TxPoolRuntimeApi<Block>,
    P: TransactionPool<Block = Block> + 'static,
    A: ChainApi<Block = Block> + 'static,
    SC: SelectChain<Block> + 'static,

    C::Api: sp_consensus_babe::BabeApi<Block>,
    C::Api: committee_rpc::CmStorageRuntimeApi<Block, AccountId>,
    C::Api: simple_rpc_rpc::SrStorageRuntimeApi<Block, AccountId, Balance>,
    C::Api: online_profile_rpc::OpStorageRuntimeApi<Block, AccountId, Balance, BlockNumber>,
    C::Api: online_committee_rpc::OcStorageRuntimeApi<Block, AccountId, BlockNumber, Balance>,
    C::Api: rent_machine_rpc::RmStorageRuntimeApi<Block, AccountId, BlockNumber, Balance>,
    C::Api: terminating_rental_rpc::IrStorageRuntimeApi<Block, AccountId, Balance, BlockNumber>,
{
    use dbc_client_rpc_debug::{Debug, DebugServer};
    use dbc_client_rpc_trace::{Trace, TraceServer};
    use dbc_client_rpc_txpool::{TxPool, TxPoolServer};
    use fc_rpc::{
        Eth, EthApiServer, EthFilter, EthFilterApiServer, EthPubSub, EthPubSubApiServer, Net,
        NetApiServer, Web3, Web3ApiServer,
    };
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
    use sc_consensus_babe_rpc::{Babe, BabeApiServer};
    use sc_consensus_grandpa_rpc::{Grandpa, GrandpaApiServer};
    use substrate_frame_rpc_system::{System, SystemApiServer};

    use committee_rpc::{CmRpcApiServer, CmStorage};
    use online_committee_rpc::{OcRpcApiServer, OcStorage};
    use online_profile_rpc::{OpRpcApiServer, OpStorage};
    use rent_machine_rpc::{RmRpcApiServer, RmStorage};
    use simple_rpc_rpc::{SimpleRpcApiServer, SrStorage};
    use terminating_rental_rpc::{IrRpcApiServer, IrStorage};

    let mut io = RpcModule::new(());
    let FullDeps {
        client,
        pool,
        select_chain,
        chain_spec: _,
        deny_unsafe,
        graph,
        network,
        filter_pool,
        ethapi_cmd,
        frontier_backend,
        backend: _,
        is_authority,
        overrides,
        block_data_cache,
        fee_history_limit,
        fee_history_cache,
        babe,
        grandpa,
        max_past_logs,
        logs_request_timeout: _,
        forced_parent_hashes,
        sync_service,
    } = deps;

    let BabeDeps { keystore, babe_worker_handle } = babe;
    let GrandpaDeps {
        shared_voter_state,
        shared_authority_set,
        justification_stream,
        subscription_executor,
        finality_provider,
    } = grandpa;

    io.merge(System::new(Arc::clone(&client), Arc::clone(&pool), deny_unsafe).into_rpc())
        .ok();
    io.merge(TransactionPayment::new(Arc::clone(&client)).into_rpc()).ok();
    io.merge(
        Babe::new(client.clone(), babe_worker_handle.clone(), keystore, select_chain, deny_unsafe)
            .into_rpc(),
    )
    .ok();
    io.merge(
        Grandpa::new(
            Arc::clone(&subscription_executor),
            shared_authority_set.clone(),
            shared_voter_state,
            justification_stream,
            finality_provider,
        )
        .into_rpc(),
    )
    .ok();

    io.merge(SrStorage::new(client.clone()).into_rpc()).ok();
    io.merge(CmStorage::new(client.clone()).into_rpc()).ok();
    io.merge(OcStorage::new(client.clone()).into_rpc()).ok();
    io.merge(OpStorage::new(client.clone()).into_rpc()).ok();
    io.merge(RmStorage::new(client.clone()).into_rpc()).ok();
    io.merge(IrStorage::new(client.clone()).into_rpc()).ok();
    io.merge(
        EthFilter::new(
            client.clone(),
            frontier_backend.clone(),
            fc_rpc::TxPool::new(client.clone(), graph.clone()),
            filter_pool,
            500_usize, // max stored filters
            max_past_logs,
            block_data_cache.clone(),
        )
        .into_rpc(),
    )
    .ok();

    io.merge(
        Net::new(
            Arc::clone(&client),
            network.clone(),
            // Whether to format the `peer_count` response as Hex (default) or not.
            true,
        )
        .into_rpc(),
    )
    .ok();

    io.merge(Web3::new(Arc::clone(&client)).into_rpc()).ok();

    io.merge(
        EthPubSub::new(
            Arc::clone(&pool),
            Arc::clone(&client),
            sync_service.clone(),
            Arc::clone(&subscription_executor),
            Arc::clone(&overrides),
            pubsub_notification_sinks.clone(),
        )
        .into_rpc(),
    )
    .ok();

    if ethapi_cmd.contains(&EthApiCmd::Txpool) {
        io.merge(TxPool::new(Arc::clone(&client), graph.clone()).into_rpc()).ok();
    }

    // Nor any signers
    let signers = Vec::new();

    enum Never {}
    impl<T> fp_rpc::ConvertTransaction<T> for Never {
        fn convert_transaction(&self, _transaction: pallet_ethereum::Transaction) -> T {
            // The Never type is not instantiable, but this method requires the type to be
            // instantiated to be called (`&self` parameter), so if the code compiles we have the
            // guarantee that this function will never be called.
            unreachable!()
        }
    }
    let convert_transaction: Option<Never> = None;

    io.merge(
        Eth::new(
            Arc::clone(&client),
            Arc::clone(&pool),
            graph.clone(),
            convert_transaction,
            Arc::clone(&sync_service),
            signers,
            Arc::clone(&overrides),
            Arc::clone(&frontier_backend),
            is_authority,
            Arc::clone(&block_data_cache),
            fee_history_cache,
            fee_history_limit,
            10,
            forced_parent_hashes,
        )
        .into_rpc(),
    )
    .ok();

    if let Some(tracing_config) = maybe_tracing_config {
        if let Some(trace_filter_requester) = tracing_config.tracing_requesters.trace {
            io.merge(
                Trace::new(client, trace_filter_requester, tracing_config.trace_filter_max_count)
                    .into_rpc(),
            )
            .ok();
        }

        if let Some(debug_requester) = tracing_config.tracing_requesters.debug {
            io.merge(Debug::new(debug_requester).into_rpc()).ok();
        }
    }

    Ok(io)
}
