#![warn(missing_docs)]
#![warn(unused_crate_dependencies)]

use std::sync::Arc;

use dbc_primitives::{AccountId, Balance, Block, BlockNumber, Hash, Index};
use jsonrpsee::RpcModule;
use sc_client_api::AuxStore;
use sc_consensus_babe::BabeWorkerHandle;
use sc_consensus_grandpa::{
    FinalityProofProvider, GrandpaJustificationStream, SharedAuthoritySet, SharedVoterState,
};
use sc_rpc::SubscriptionTaskExecutor;
pub use sc_rpc_api::DenyUnsafe;
use sc_transaction_pool::ChainApi;
use sc_transaction_pool_api::TransactionPool;
use sp_api::ProvideRuntimeApi;
use sp_block_builder::BlockBuilder;
use sp_blockchain::{Error as BlockChainError, HeaderBackend, HeaderMetadata};
use sp_consensus::SelectChain;
use sp_consensus_babe::BabeApi;
use sp_keystore::KeystorePtr;
use sp_runtime::traits::Block as BlockT;

mod eth;
pub use self::eth::{create_eth, overrides_handle, EthDeps};

/// Extra dependencies for BABE.
pub struct BabeDeps {
    /// A handle to the BABE worker for issuing requests.
    pub babe_worker_handle: BabeWorkerHandle<Block>,
    /// The keystore that manages the keys of the node.
    pub keystore: KeystorePtr,
}

/// Extra dependencies for GRANDPA
pub struct GrandpaDeps<B> {
    /// Voting round info.
    pub shared_voter_state: SharedVoterState,
    /// Authority set info.
    pub shared_authority_set: SharedAuthoritySet<Hash, BlockNumber>,
    /// Receives notifications about justification events from Grandpa.
    pub justification_stream: GrandpaJustificationStream<Block>,
    /// Executor to drive the subscription manager in the Grandpa RPC handler.
    pub subscription_executor: SubscriptionTaskExecutor,
    /// Finality proof provider.
    pub finality_provider: Arc<FinalityProofProvider<B, Block>>,
}

/// Full client dependencies.
pub struct FullDeps<C, P, SC, B, A: ChainApi, CT> {
    /// The client instance to use.
    pub client: Arc<C>,
    /// Transaction pool instance.
    pub pool: Arc<P>,
    /// The SelectChain Strategy
    pub select_chain: SC,
    /// A copy of the chain spec.
    pub chain_spec: Box<dyn sc_chain_spec::ChainSpec>,
    /// Whether to deny unsafe calls
    pub deny_unsafe: DenyUnsafe,
    /// BABE specific dependencies.
    pub babe: BabeDeps,
    /// GRANDPA specific dependencies.
    pub grandpa: GrandpaDeps<B>,
    /// Ethereum-compatibility specific dependencies.
    pub eth: EthDeps<C, P, A, CT, Block>,
}

pub struct DefaultEthConfig<C, BE>(std::marker::PhantomData<(C, BE)>);

impl<C, BE> fc_rpc::EthConfig<Block, C> for DefaultEthConfig<C, BE>
where
    C: sc_client_api::StorageProvider<Block, BE> + Sync + Send + 'static,
    BE: sc_client_api::Backend<Block> + 'static,
{
    type EstimateGasAdapter = ();
    type RuntimeStorageOverride =
        fc_rpc::frontier_backend_client::SystemAccountId20StorageOverride<Block, C, BE>;
}

/// Instantiate all Full RPC extensions.
pub fn create_full<C, P, SC, BE, A, CT>(
    deps: FullDeps<C, P, SC, BE, A, CT>,
    subscription_task_executor: SubscriptionTaskExecutor,
    pubsub_notification_sinks: Arc<
        fc_mapping_sync::EthereumBlockNotificationSinks<
            fc_mapping_sync::EthereumBlockNotification<Block>,
        >,
    >,
) -> Result<RpcModule<()>, Box<dyn std::error::Error + Send + Sync>>
where
    C: ProvideRuntimeApi<Block>
        + sc_client_api::BlockBackend<Block>
        + HeaderBackend<Block>
        + AuxStore
        + HeaderMetadata<Block, Error = BlockChainError>
        + Sync
        + Send
        + 'static,
    C::Api: substrate_frame_rpc_system::AccountNonceApi<Block, AccountId, Index>,
    C::Api: pallet_transaction_payment_rpc::TransactionPaymentRuntimeApi<Block, Balance>,
    C::Api: BabeApi<Block>,
    C::Api: BlockBuilder<Block>,
    P: TransactionPool + 'static,
    SC: SelectChain<Block> + 'static,
    BE: sc_client_api::Backend<Block> + Send + Sync + 'static,
    BE::State: sc_client_api::backend::StateBackend<sp_runtime::traits::HashFor<Block>>,

    C: sp_api::CallApiAt<Block>
        + sc_client_api::StorageProvider<Block, BE>
        + sc_client_api::BlockchainEvents<Block>,
    C::Api: committee_rpc::CmStorageRuntimeApi<Block, AccountId>,
    C::Api: simple_rpc_rpc::SrStorageRuntimeApi<Block, AccountId, Balance>,
    C::Api: online_profile_rpc::OpStorageRuntimeApi<Block, AccountId, Balance, BlockNumber>,
    C::Api: online_committee_rpc::OcStorageRuntimeApi<Block, AccountId, BlockNumber, Balance>,
    C::Api: rent_machine_rpc::RmStorageRuntimeApi<Block, AccountId, BlockNumber, Balance>,
    C::Api: terminating_rental_rpc::IrStorageRuntimeApi<Block, AccountId, Balance, BlockNumber>,
    C::Api: fp_rpc::EthereumRuntimeRPCApi<Block>,
    C::Api: fp_rpc::ConvertTransactionRuntimeApi<Block>,
    P: TransactionPool<Block = Block> + Sync + Send + 'static,
    A: ChainApi<Block = Block> + 'static,
    CT: fp_rpc::ConvertTransaction<<Block as BlockT>::Extrinsic> + Send + Sync + 'static,
{
    use pallet_transaction_payment_rpc::{TransactionPayment, TransactionPaymentApiServer};
    use sc_consensus_babe_rpc::{Babe, BabeApiServer};
    use sc_consensus_grandpa_rpc::{Grandpa, GrandpaApiServer};
    use sc_rpc::dev::{Dev, DevApiServer};
    use sc_rpc_spec_v2::chain_spec::{ChainSpec, ChainSpecApiServer};
    use sc_sync_state_rpc::{SyncState, SyncStateApiServer};
    use substrate_frame_rpc_system::{System, SystemApiServer};

    use committee_rpc::{CmRpcApiServer, CmStorage};
    use dbc_finality_rpc::{DbcFinality, DbcFinalityApiServer};
    use online_committee_rpc::{OcRpcApiServer, OcStorage};
    use online_profile_rpc::{OpRpcApiServer, OpStorage};
    use rent_machine_rpc::{RmRpcApiServer, RmStorage};
    use simple_rpc_rpc::{SimpleRpcApiServer, SrStorage};
    use terminating_rental_rpc::{IrRpcApiServer, IrStorage};

    let mut io = RpcModule::new(());
    let FullDeps { client, pool, select_chain, chain_spec, deny_unsafe, babe, grandpa, eth } = deps;

    let BabeDeps { keystore, babe_worker_handle } = babe;
    let GrandpaDeps {
        shared_voter_state,
        shared_authority_set,
        justification_stream,
        subscription_executor,
        finality_provider,
    } = grandpa;

    let chain_name = chain_spec.name().to_string();
    let genesis_hash = client.block_hash(0).ok().flatten().expect("Genesis block exists; qed");
    let properties = chain_spec.properties();
    io.merge(ChainSpec::new(chain_name, genesis_hash, properties).into_rpc())?;

    io.merge(System::new(client.clone(), pool, deny_unsafe).into_rpc())?;
    io.merge(TransactionPayment::new(client.clone()).into_rpc())?;
    io.merge(
        Babe::new(client.clone(), babe_worker_handle.clone(), keystore, select_chain, deny_unsafe)
            .into_rpc(),
    )?;
    io.merge(
        Grandpa::new(
            subscription_executor,
            shared_authority_set.clone(),
            shared_voter_state,
            justification_stream,
            finality_provider,
        )
        .into_rpc(),
    )?;

    io.merge(
        SyncState::new(chain_spec, client.clone(), shared_authority_set, babe_worker_handle)?
            .into_rpc(),
    )?;

    io.merge(Dev::new(client.clone(), deny_unsafe).into_rpc())?;

    io.merge(SrStorage::new(client.clone()).into_rpc())?;
    io.merge(CmStorage::new(client.clone()).into_rpc())?;
    io.merge(OcStorage::new(client.clone()).into_rpc())?;
    io.merge(OpStorage::new(client.clone()).into_rpc())?;
    io.merge(RmStorage::new(client.clone()).into_rpc())?;
    io.merge(IrStorage::new(client.clone()).into_rpc())?;

    io.merge(DbcFinality::new(client.clone(), eth.frontier_backend.clone()).into_rpc())?;

    // Ethereum compatibility RPCs
    let io = create_eth::<_, _, _, _, _, _, DefaultEthConfig<C, BE>>(
        io,
        eth,
        subscription_task_executor,
        pubsub_notification_sinks,
    )?;

    Ok(io)
}
