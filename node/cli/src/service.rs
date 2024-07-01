// This file is part of Substrate.

// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: GPL-3.0-or-later WITH Classpath-exception-2.0

// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

#![warn(unused_extern_crates)]

//! Service implementation. Specialized wrapper over substrate service.

use crate::Cli;
use dbc_executor::DBCExecutorDispatch;
use dbc_primitives::Block;
use dbc_runtime::{RuntimeApi, TransactionConverter};
use fc_db::DatabaseSource;
use fc_mapping_sync::{MappingSyncWorker, SyncStrategy};
use fc_rpc::EthTask;
use fc_rpc_core::types::{FeeHistoryCache, FilterPool};
use frame_benchmarking_cli::SUBSTRATE_REFERENCE_HARDWARE;
use frame_system_rpc_runtime_api::AccountNonceApi;
use futures::prelude::*;
use parity_scale_codec::Encode;
use sc_client_api::{BlockBackend, BlockchainEvents};
use sc_consensus_babe::{self, SlotProportion};
use sc_executor::NativeElseWasmExecutor;
use sc_network::NetworkService;
use sc_network_common::{
    protocol::event::Event, service::NetworkEventStream, sync::warp::WarpSyncParams,
};
use sc_rpc::SubscriptionTaskExecutor;
use sc_service::{
    config::Configuration, error::Error as ServiceError, BasePath, RpcHandlers, TaskManager,
};
use sc_telemetry::{Telemetry, TelemetryWorker};
use sp_api::ProvideRuntimeApi;
use sp_core::crypto::Pair;
use sp_runtime::{generic, traits::Block as BlockT, SaturatedConversion};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};

/// The full client type definition.
pub type FullClient =
    sc_service::TFullClient<Block, RuntimeApi, NativeElseWasmExecutor<DBCExecutorDispatch>>;
type FullBackend = sc_service::TFullBackend<Block>;
type FullSelectChain = sc_consensus::LongestChain<FullBackend, Block>;
type FullGrandpaBlockImport =
    sc_finality_grandpa::GrandpaBlockImport<FullBackend, Block, FullClient, FullSelectChain>;

/// The transaction pool type defintion.
pub type TransactionPool = sc_transaction_pool::FullPool<Block, FullClient>;

/// Fetch the nonce of the given `account` from the chain state.
///
/// Note: Should only be used for tests.
pub fn fetch_nonce(client: &FullClient, account: sp_core::sr25519::Pair) -> u32 {
    let best_hash = client.chain_info().best_hash;
    client
        .runtime_api()
        .account_nonce(best_hash, account.public().into())
        .expect("Fetching account nonce works; qed")
}

/// Create a transaction using the given `call`.
///
/// The transaction will be signed by `sender`. If `nonce` is `None` it will be fetched from the
/// state of the best block.
///
/// Note: Should only be used for tests.
pub fn create_extrinsic(
    client: &FullClient,
    sender: sp_core::sr25519::Pair,
    function: impl Into<dbc_runtime::RuntimeCall>,
    nonce: Option<u32>,
) -> dbc_runtime::UncheckedExtrinsic {
    let function = function.into();
    let genesis_hash = client.block_hash(0).ok().flatten().expect("Genesis block exists; qed");
    let best_hash = client.chain_info().best_hash;
    let best_block = client.chain_info().best_number;
    let nonce = nonce.unwrap_or_else(|| fetch_nonce(client, sender.clone()));

    let period = dbc_runtime::BlockHashCount::get()
        .checked_next_power_of_two()
        .map(|c| c / 2)
        .unwrap_or(2) as u64;
    let tip = 0;
    let extra: dbc_runtime::SignedExtra = (
        frame_system::CheckNonZeroSender::<dbc_runtime::Runtime>::new(),
        frame_system::CheckSpecVersion::<dbc_runtime::Runtime>::new(),
        frame_system::CheckTxVersion::<dbc_runtime::Runtime>::new(),
        frame_system::CheckGenesis::<dbc_runtime::Runtime>::new(),
        frame_system::CheckEra::<dbc_runtime::Runtime>::from(generic::Era::mortal(
            period,
            best_block.saturated_into(),
        )),
        frame_system::CheckNonce::<dbc_runtime::Runtime>::from(nonce),
        frame_system::CheckWeight::<dbc_runtime::Runtime>::new(),
        pallet_asset_tx_payment::ChargeAssetTxPayment::<dbc_runtime::Runtime>::from(tip, None),
    );

    let raw_payload = dbc_runtime::SignedPayload::from_raw(
        function.clone(),
        extra.clone(),
        (
            (),
            dbc_runtime::VERSION.spec_version,
            dbc_runtime::VERSION.transaction_version,
            genesis_hash,
            best_hash,
            (),
            (),
            (),
        ),
    );
    let signature = raw_payload.using_encoded(|e| sender.sign(e));

    dbc_runtime::UncheckedExtrinsic::new_signed(
        function,
        sp_runtime::AccountId32::from(sender.public()).into(),
        dbc_runtime::Signature::Sr25519(signature),
        extra,
    )
}

/// Create the frontier database directory.
pub fn frontier_database_dir(config: &Configuration, path: &str) -> std::path::PathBuf {
    let config_dir = config
        .base_path
        .as_ref()
        .map(|base_path| base_path.config_dir(config.chain_spec.id()))
        .unwrap_or_else(|| {
            BasePath::from_project("", "", "dbc").config_dir(config.chain_spec.id())
        });
    config_dir.join("frontier").join(path)
}

/// Open the frontier backend.
pub fn open_frontier_backend<C>(
    client: Arc<C>,
    config: &Configuration,
) -> Result<Arc<fc_db::Backend<Block>>, String>
where
    C: sp_blockchain::HeaderBackend<Block>,
{
    Ok(Arc::new(fc_db::Backend::<Block>::new(
        client,
        &fc_db::DatabaseSettings {
            source: match config.database {
                DatabaseSource::RocksDb { .. } => DatabaseSource::RocksDb {
                    path: frontier_database_dir(config, "db"),
                    cache_size: 0,
                },
                DatabaseSource::ParityDb { .. } =>
                    DatabaseSource::ParityDb { path: frontier_database_dir(config, "paritydb") },
                DatabaseSource::Auto { .. } => DatabaseSource::Auto {
                    rocksdb_path: frontier_database_dir(config, "db"),
                    paritydb_path: frontier_database_dir(config, "paritydb"),
                    cache_size: 0,
                },
                _ =>
                    return Err("Supported db sources: `rocksdb` | `paritydb` | `auto`".to_string()),
            },
        },
    )?))
}

/// Creates a new partial node.
pub fn new_partial(
    config: &mut Configuration,
) -> Result<
    sc_service::PartialComponents<
        FullClient,
        FullBackend,
        FullSelectChain,
        sc_consensus::DefaultImportQueue<Block, FullClient>,
        sc_transaction_pool::FullPool<Block, FullClient>,
        (
            (
                sc_consensus_babe::BabeBlockImport<Block, FullClient, FullGrandpaBlockImport>,
                sc_finality_grandpa::LinkHalf<Block, FullClient, FullSelectChain>,
                sc_consensus_babe::BabeLink<Block>,
            ),
            Option<Telemetry>,
            (Option<FilterPool>, FeeHistoryCache, Arc<fc_db::Backend<Block>>),
        ),
    >,
    ServiceError,
> {
    // Use ethereum style for subscription ids
    config.rpc_id_provider = Some(Box::new(fc_rpc::EthereumSubIdProvider));

    let telemetry = config
        .telemetry_endpoints
        .clone()
        .filter(|x| !x.is_empty())
        .map(|endpoints| -> Result<_, sc_telemetry::Error> {
            let worker = TelemetryWorker::new(16)?;
            let telemetry = worker.handle().new_telemetry(endpoints);
            Ok((worker, telemetry))
        })
        .transpose()?;

    let executor = NativeElseWasmExecutor::<DBCExecutorDispatch>::new(
        config.wasm_method,
        config.default_heap_pages,
        config.max_runtime_instances,
        config.runtime_cache_size,
    );

    let (client, backend, keystore_container, task_manager) =
        sc_service::new_full_parts::<Block, RuntimeApi, _>(
            config,
            telemetry.as_ref().map(|(_, telemetry)| telemetry.handle()),
            executor,
        )?;
    let client = Arc::new(client);

    let telemetry = telemetry.map(|(worker, telemetry)| {
        task_manager.spawn_handle().spawn("telemetry", None, worker.run());
        telemetry
    });

    let select_chain = sc_consensus::LongestChain::new(backend.clone());

    let transaction_pool = sc_transaction_pool::BasicPool::new_full(
        config.transaction_pool.clone(),
        config.role.is_authority().into(),
        config.prometheus_registry(),
        task_manager.spawn_essential_handle(),
        client.clone(),
    );

    let filter_pool: Option<FilterPool> = Some(Arc::new(Mutex::new(BTreeMap::new())));
    let fee_history_cache: FeeHistoryCache = Arc::new(Mutex::new(BTreeMap::new()));
    let frontier_backend = open_frontier_backend(client.clone(), config)?;

    let (grandpa_block_import, grandpa_link) = sc_finality_grandpa::block_import(
        client.clone(),
        &(client.clone() as Arc<_>),
        select_chain.clone(),
        telemetry.as_ref().map(|x| x.handle()),
    )?;
    let justification_import = grandpa_block_import.clone();

    let (babe_block_import, babe_link) = sc_consensus_babe::block_import(
        sc_consensus_babe::configuration(&*client)?,
        grandpa_block_import,
        client.clone(),
    )?;

    let slot_duration = babe_link.config().slot_duration();
    let import_queue = sc_consensus_babe::import_queue(
        babe_link.clone(),
        babe_block_import.clone(),
        Some(Box::new(justification_import)),
        client.clone(),
        select_chain.clone(),
        move |_, ()| async move {
            let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

            let slot =
				sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
					*timestamp,
					slot_duration,
				);

            Ok((slot, timestamp))
        },
        &task_manager.spawn_essential_handle(),
        config.prometheus_registry(),
        telemetry.as_ref().map(|x| x.handle()),
    )?;

    let import_setup = (babe_block_import, grandpa_link, babe_link);
    let frontier_setup = (filter_pool, fee_history_cache, frontier_backend);

    Ok(sc_service::PartialComponents {
        client,
        backend,
        task_manager,
        keystore_container,
        select_chain,
        import_queue,
        transaction_pool,
        other: (import_setup, telemetry, frontier_setup),
    })
}

/// Result of [`new_full_base`].
pub struct NewFullBase {
    /// The task manager of the node.
    pub task_manager: TaskManager,
    /// The client instance of the node.
    pub client: Arc<FullClient>,
    /// The networking service of the node.
    pub network: Arc<NetworkService<Block, <Block as BlockT>::Hash>>,
    /// The transaction pool of the node.
    pub transaction_pool: Arc<TransactionPool>,
    /// The rpc handlers of the node.
    pub rpc_handlers: RpcHandlers,
}

/// Creates a full service from the configuration.
pub fn new_full_base(
    mut config: Configuration,
    disable_hardware_benchmarks: bool,
    with_startup_data: impl FnOnce(
        &sc_consensus_babe::BabeBlockImport<Block, FullClient, FullGrandpaBlockImport>,
        &sc_consensus_babe::BabeLink<Block>,
    ),
) -> Result<NewFullBase, ServiceError> {
    let hwbench = (!disable_hardware_benchmarks)
        .then_some(config.database.path().map(|database_path| {
            let _ = std::fs::create_dir_all(&database_path);
            sc_sysinfo::gather_hwbench(Some(database_path))
        }))
        .flatten();

    let sc_service::PartialComponents {
        client,
        backend,
        mut task_manager,
        import_queue,
        keystore_container,
        select_chain,
        transaction_pool,
        other: (import_setup, mut telemetry, frontier_setup),
    } = new_partial(&mut config)?;

    let (babe_block_import, grandpa_link, babe_link) = import_setup;
    let (filter_pool, fee_history_cache, frontier_backend) = frontier_setup;

    let auth_disc_publish_non_global_ips = config.network.allow_non_globals_in_dht;
    let grandpa_protocol_name = sc_finality_grandpa::protocol_standard_name(
        &client.block_hash(0).ok().flatten().expect("Genesis block exists; qed"),
        &config.chain_spec,
    );

    config
        .network
        .extra_sets
        .push(sc_finality_grandpa::grandpa_peers_set_config(grandpa_protocol_name.clone()));
    let warp_sync = Arc::new(sc_finality_grandpa::warp_proof::NetworkProvider::new(
        backend.clone(),
        grandpa_link.shared_authority_set().clone(),
        Vec::default(),
    ));

    let (network, system_rpc_tx, tx_handler_controller, network_starter) =
        sc_service::build_network(sc_service::BuildNetworkParams {
            config: &config,
            client: client.clone(),
            transaction_pool: transaction_pool.clone(),
            spawn_handle: task_manager.spawn_handle(),
            import_queue,
            block_announce_validator_builder: None,
            warp_sync_params: Some(WarpSyncParams::WithProvider(warp_sync)),
        })?;

    if config.offchain_worker.enabled {
        sc_service::build_offchain_workers(
            &config,
            task_manager.spawn_handle(),
            client.clone(),
            network.clone(),
        );
    }

    let role = config.role.clone();
    let force_authoring = config.force_authoring;
    let backoff_authoring_blocks =
        Some(sc_consensus_slots::BackoffAuthoringOnFinalizedHeadLagging::default());
    let name = config.network.node_name.clone();
    let enable_grandpa = !config.disable_grandpa;
    let prometheus_registry = config.prometheus_registry().cloned();

    // EVM
    let overrides = dbc_rpc::overrides_handle(client.clone());
    let fee_history_cache_limit = 2048;
    let block_data_cache = Arc::new(fc_rpc::EthBlockDataCacheTask::new(
        task_manager.spawn_handle(),
        overrides.clone(),
        50,
        50,
        prometheus_registry.clone(),
    ));

    let rpc_builder = {
        let justification_stream = grandpa_link.justification_stream();
        let shared_authority_set = grandpa_link.shared_authority_set().clone();

        let finality_proof_provider = sc_finality_grandpa::FinalityProofProvider::new_for_service(
            backend.clone(),
            Some(shared_authority_set.clone()),
        );

        let babe_config = babe_link.config().clone();
        let shared_epoch_changes = babe_link.epoch_changes().clone();

        let client = client.clone();
        let transaction_pool_clone = transaction_pool.clone();
        let select_chain = select_chain.clone();
        let keystore = keystore_container.sync_keystore();
        let chain_spec = config.chain_spec.cloned_box();

        // EVM
        let network = network.clone();
        let filter_pool = filter_pool.clone();
        let frontier_backend = frontier_backend.clone();
        let overrides = overrides.clone();
        let fee_history_cache = fee_history_cache.clone();
        let backend_clone = backend.clone();

        Box::new(move |deny_unsafe, subscription_task_executor: SubscriptionTaskExecutor| {
            let deps = dbc_rpc::FullDeps {
                client: client.clone(),
                pool: transaction_pool_clone.clone(),
                select_chain: select_chain.clone(),
                chain_spec: chain_spec.cloned_box(),
                deny_unsafe,
                babe: dbc_rpc::BabeDeps {
                    babe_config: babe_config.clone(),
                    shared_epoch_changes: shared_epoch_changes.clone(),
                    keystore: keystore.clone(),
                },
                grandpa: dbc_rpc::GrandpaDeps {
                    shared_voter_state: sc_finality_grandpa::SharedVoterState::empty(),
                    shared_authority_set: shared_authority_set.clone(),
                    justification_stream: justification_stream.clone(),
                    subscription_executor: subscription_task_executor.clone(),
                    finality_provider: finality_proof_provider.clone(),
                },
                eth: dbc_rpc::EthDeps {
                    client: client.clone(),
                    pool: transaction_pool_clone.clone(),
                    graph: transaction_pool_clone.pool().clone(),
                    converter: Some(TransactionConverter),
                    is_authority: false,
                    enable_dev_signer: false, // eth_config.enable_dev_signer,
                    network: network.clone(),
                    frontier_backend: frontier_backend.clone(),
                    overrides: overrides.clone(),
                    block_data_cache: block_data_cache.clone(),
                    filter_pool: filter_pool.clone(),
                    max_past_logs: 10000, // eth_config.max_past_logs,
                    fee_history_cache: fee_history_cache.clone(),
                    fee_history_cache_limit,
                    execute_gas_limit_multiplier: 10, // eth_config.execute_gas_limit_multiplier,
                },
            };

            dbc_rpc::create_full(deps, backend_clone.clone(), subscription_task_executor.clone())
                .map_err(Into::into)
        })
    };

    let rpc_handlers = sc_service::spawn_tasks(sc_service::SpawnTasksParams {
        config,
        backend: backend.clone(),
        client: client.clone(),
        keystore: keystore_container.sync_keystore(),
        network: network.clone(),
        rpc_builder: Box::new(rpc_builder),
        transaction_pool: transaction_pool.clone(),
        task_manager: &mut task_manager,
        system_rpc_tx,
        tx_handler_controller,
        telemetry: telemetry.as_mut(),
    })?;

    if let Some(hwbench) = hwbench {
        sc_sysinfo::print_hwbench(&hwbench);
        if !SUBSTRATE_REFERENCE_HARDWARE.check_hardware(&hwbench) && role.is_authority() {
            log::warn!(
                "⚠️  The hardware does not meet the minimal requirements for role 'Authority'."
            );
        }

        if let Some(ref mut telemetry) = telemetry {
            let telemetry_handle = telemetry.handle();
            task_manager.spawn_handle().spawn(
                "telemetry_hwbench",
                None,
                sc_sysinfo::initialize_hwbench_telemetry(telemetry_handle, hwbench),
            );
        }
    }

    (with_startup_data)(&babe_block_import, &babe_link);

    task_manager.spawn_essential_handle().spawn(
        "frontier-mapping-sync-worker",
        Some("frontier"),
        MappingSyncWorker::new(
            client.import_notification_stream(),
            Duration::new(6, 0),
            client.clone(),
            backend,
            overrides.clone(),
            frontier_backend.clone(),
            3,
            0,
            SyncStrategy::Normal,
        )
        .for_each(|()| futures::future::ready(())),
    );

    // Spawn Frontier EthFilterApi maintenance task.
    if let Some(filter_pool) = filter_pool {
        // Each filter is allowed to stay in the pool for 100 blocks.
        const FILTER_RETAIN_THRESHOLD: u64 = 100;
        task_manager.spawn_essential_handle().spawn(
            "frontier-filter-pool",
            Some("frontier"),
            EthTask::filter_pool_task(Arc::clone(&client), filter_pool, FILTER_RETAIN_THRESHOLD),
        );
    }

    // Spawn Frontier FeeHistory cache maintenance task.
    task_manager.spawn_essential_handle().spawn(
        "frontier-fee-history",
        Some("frontier"),
        EthTask::fee_history_task(
            Arc::clone(&client),
            Arc::clone(&overrides),
            fee_history_cache,
            fee_history_cache_limit,
        ),
    );

    if let sc_service::config::Role::Authority { .. } = &role {
        let proposer = sc_basic_authorship::ProposerFactory::new(
            task_manager.spawn_handle(),
            client.clone(),
            transaction_pool.clone(),
            prometheus_registry.as_ref(),
            telemetry.as_ref().map(|x| x.handle()),
        );

        let client_clone = client.clone();
        let slot_duration = babe_link.config().slot_duration();
        let babe_config = sc_consensus_babe::BabeParams {
            keystore: keystore_container.sync_keystore(),
            client: client.clone(),
            select_chain,
            env: proposer,
            block_import: babe_block_import,
            sync_oracle: network.clone(),
            justification_sync_link: network.clone(),
            create_inherent_data_providers: move |parent, ()| {
                let client_clone = client_clone.clone();
                async move {
                    let timestamp = sp_timestamp::InherentDataProvider::from_system_time();

                    let slot =
						sp_consensus_babe::inherents::InherentDataProvider::from_timestamp_and_slot_duration(
							*timestamp,
							slot_duration,
						);

                    let storage_proof =
                        sp_transaction_storage_proof::registration::new_data_provider(
                            &*client_clone,
                            &parent,
                        )?;

                    Ok((slot, timestamp, storage_proof))
                }
            },
            force_authoring,
            backoff_authoring_blocks,
            babe_link,
            block_proposal_slot_portion: SlotProportion::new(0.5),
            max_block_proposal_slot_portion: None,
            telemetry: telemetry.as_ref().map(|x| x.handle()),
        };

        let babe = sc_consensus_babe::start_babe(babe_config)?;
        task_manager.spawn_essential_handle().spawn_blocking(
            "babe-proposer",
            Some("block-authoring"),
            babe,
        );
    }

    // Spawn authority discovery module.
    if role.is_authority() {
        let authority_discovery_role =
            sc_authority_discovery::Role::PublishAndDiscover(keystore_container.keystore());
        let dht_event_stream =
            network.event_stream("authority-discovery").filter_map(|e| async move {
                match e {
                    Event::Dht(e) => Some(e),
                    _ => None,
                }
            });
        let (authority_discovery_worker, _service) =
            sc_authority_discovery::new_worker_and_service_with_config(
                sc_authority_discovery::WorkerConfig {
                    publish_non_global_ips: auth_disc_publish_non_global_ips,
                    ..Default::default()
                },
                client.clone(),
                network.clone(),
                Box::pin(dht_event_stream),
                authority_discovery_role,
                prometheus_registry.clone(),
            );

        task_manager.spawn_handle().spawn(
            "authority-discovery-worker",
            Some("networking"),
            authority_discovery_worker.run(),
        );
    }

    // if the node isn't actively participating in consensus then it doesn't
    // need a keystore, regardless of which protocol we use below.
    let keystore =
        if role.is_authority() { Some(keystore_container.sync_keystore()) } else { None };

    let config = sc_finality_grandpa::Config {
        // FIXME #1578 make this available through chainspec
        gossip_duration: std::time::Duration::from_millis(333),
        justification_period: 512,
        name: Some(name),
        observer_enabled: false,
        keystore,
        local_role: role,
        telemetry: telemetry.as_ref().map(|x| x.handle()),
        protocol_name: grandpa_protocol_name,
    };

    if enable_grandpa {
        // start the full GRANDPA voter
        // NOTE: non-authorities could run the GRANDPA observer protocol, but at
        // this point the full voter should provide better guarantees of block
        // and vote data availability than the observer. The observer has not
        // been tested extensively yet and having most nodes in a network run it
        // could lead to finality stalls.
        let grandpa_config = sc_finality_grandpa::GrandpaParams {
            config,
            link: grandpa_link,
            network: network.clone(),
            telemetry: telemetry.as_ref().map(|x| x.handle()),
            voting_rule: sc_finality_grandpa::VotingRulesBuilder::default().build(),
            prometheus_registry,
            shared_voter_state: sc_finality_grandpa::SharedVoterState::empty(),
        };

        // the GRANDPA voter task is considered infallible, i.e.
        // if it fails we take down the service with it.
        task_manager.spawn_essential_handle().spawn_blocking(
            "grandpa-voter",
            None,
            sc_finality_grandpa::run_grandpa_voter(grandpa_config)?,
        );
    }

    network_starter.start_network();
    Ok(NewFullBase { task_manager, client, network, transaction_pool, rpc_handlers })
}

/// Builds a new service for a full client.
pub fn new_full(config: Configuration, cli: Cli) -> Result<TaskManager, ServiceError> {
    let database_source = config.database.clone();
    let task_manager = new_full_base(config, cli.no_hardware_benchmarks, |_, _| ())
        .map(|NewFullBase { task_manager, .. }| task_manager)?;

    sc_storage_monitor::StorageMonitorService::try_spawn(
        cli.storage_monitor,
        database_source,
        &task_manager.spawn_essential_handle(),
    )?;

    Ok(task_manager)
}

#[cfg(test)]
mod tests {
    use crate::service::{new_full_base, NewFullBase};
    use codec::Encode;
    use dbc_primitives::{Block, DigestItem, Signature};
    use dbc_runtime::{
        constants::{currency::CENTS, time::SLOT_DURATION},
        Address, BalancesCall, RuntimeCall, UncheckedExtrinsic,
    };
    use sc_client_api::BlockBackend;
    use sc_consensus::{BlockImport, BlockImportParams, ForkChoiceStrategy};
    use sc_consensus_babe::{BabeIntermediate, CompatibleDigestItem, INTERMEDIATE_KEY};
    use sc_consensus_epochs::descendent_query;
    use sc_keystore::LocalKeystore;
    use sc_service_test::TestNetNode;
    use sc_transaction_pool_api::{ChainEvent, MaintainedTransactionPool};
    use sp_consensus::{BlockOrigin, Environment, Proposer};
    use sp_core::{crypto::Pair as CryptoPair, Public};
    use sp_inherents::InherentDataProvider;
    use sp_keyring::AccountKeyring;
    use sp_keystore::{SyncCryptoStore, SyncCryptoStorePtr};
    use sp_runtime::{
        generic::{Digest, Era, SignedPayload},
        key_types::BABE,
        traits::{Block as BlockT, Header as HeaderT, IdentifyAccount, Verify},
        RuntimeAppPublic,
    };
    use sp_timestamp;
    use std::sync::Arc;

    type AccountPublic = <Signature as Verify>::Signer;

    #[test]
    // It is "ignored", but the node-cli ignored tests are running on the CI.
    // This can be run locally with `cargo test --release -p node-cli test_sync -- --ignored`.
    #[ignore]
    fn test_sync() {
        sp_tracing::try_init_simple();

        let keystore_path = tempfile::tempdir().expect("Creates keystore path");
        let keystore: SyncCryptoStorePtr =
            Arc::new(LocalKeystore::open(keystore_path.path(), None).expect("Creates keystore"));
        let alice: sp_consensus_babe::AuthorityId =
            SyncCryptoStore::sr25519_generate_new(&*keystore, BABE, Some("//Alice"))
                .expect("Creates authority pair")
                .into();

        let chain_spec = crate::chain_spec::tests::integration_test_config_with_single_authority();

        // For the block factory
        let mut slot = 1u64;

        // For the extrinsics factory
        let bob = Arc::new(AccountKeyring::Bob.pair());
        let charlie = Arc::new(AccountKeyring::Charlie.pair());
        let mut index = 0;

        sc_service_test::sync(
            chain_spec,
            |config| {
                let mut setup_handles = None;
                let NewFullBase { task_manager, client, network, transaction_pool, .. } =
                    new_full_base(
                        config,
                        false,
                        |block_import: &sc_consensus_babe::BabeBlockImport<Block, _, _>,
                         babe_link: &sc_consensus_babe::BabeLink<Block>| {
                            setup_handles = Some((block_import.clone(), babe_link.clone()));
                        },
                    )?;

                let node = sc_service_test::TestNetComponents::new(
                    task_manager,
                    client,
                    network,
                    transaction_pool,
                );
                Ok((node, setup_handles.unwrap()))
            },
            |service, &mut (ref mut block_import, ref babe_link)| {
                let parent_hash = service.client().chain_info().best_hash;
                let parent_header = service.client().header(parent_hash).unwrap().unwrap();
                let parent_number = *parent_header.number();

                futures::executor::block_on(service.transaction_pool().maintain(
                    ChainEvent::NewBestBlock { hash: parent_header.hash(), tree_route: None },
                ));

                let mut proposer_factory = sc_basic_authorship::ProposerFactory::new(
                    service.spawn_handle(),
                    service.client(),
                    service.transaction_pool(),
                    None,
                    None,
                );

                let mut digest = Digest::default();

                // even though there's only one authority some slots might be empty,
                // so we must keep trying the next slots until we can claim one.
                let (babe_pre_digest, epoch_descriptor) = loop {
                    let epoch_descriptor = babe_link
                        .epoch_changes()
                        .shared_data()
                        .epoch_descriptor_for_child_of(
                            descendent_query(&*service.client()),
                            &parent_hash,
                            parent_number,
                            slot.into(),
                        )
                        .unwrap()
                        .unwrap();

                    let epoch = babe_link
                        .epoch_changes()
                        .shared_data()
                        .epoch_data(&epoch_descriptor, |slot| {
                            sc_consensus_babe::Epoch::genesis(babe_link.config(), slot)
                        })
                        .unwrap();

                    if let Some(babe_pre_digest) =
                        sc_consensus_babe::authorship::claim_slot(slot.into(), &epoch, &keystore)
                            .map(|(digest, _)| digest)
                    {
                        break (babe_pre_digest, epoch_descriptor)
                    }

                    slot += 1;
                };

                let inherent_data = futures::executor::block_on(
                    (
                        sp_timestamp::InherentDataProvider::new(
                            std::time::Duration::from_millis(SLOT_DURATION * slot).into(),
                        ),
                        sp_consensus_babe::inherents::InherentDataProvider::new(slot.into()),
                    )
                        .create_inherent_data(),
                )
                .expect("Creates inherent data");

                digest.push(<DigestItem as CompatibleDigestItem>::babe_pre_digest(babe_pre_digest));

                let new_block = futures::executor::block_on(async move {
                    let proposer = proposer_factory.init(&parent_header).await;
                    proposer
                        .unwrap()
                        .propose(inherent_data, digest, std::time::Duration::from_secs(1), None)
                        .await
                })
                .expect("Error making test block")
                .block;

                let (new_header, new_body) = new_block.deconstruct();
                let pre_hash = new_header.hash();
                // sign the pre-sealed hash of the block and then
                // add it to a digest item.
                let to_sign = pre_hash.encode();
                let signature = SyncCryptoStore::sign_with(
                    &*keystore,
                    sp_consensus_babe::AuthorityId::ID,
                    &alice.to_public_crypto_pair(),
                    &to_sign,
                )
                .unwrap()
                .unwrap()
                .try_into()
                .unwrap();
                let item = <DigestItem as CompatibleDigestItem>::babe_seal(signature);
                slot += 1;

                let mut params = BlockImportParams::new(BlockOrigin::File, new_header);
                params.post_digests.push(item);
                params.body = Some(new_body);
                params.insert_intermediate(
                    INTERMEDIATE_KEY,
                    BabeIntermediate::<Block> { epoch_descriptor },
                );
                params.fork_choice = Some(ForkChoiceStrategy::LongestChain);

                futures::executor::block_on(block_import.import_block(params, Default::default()))
                    .expect("error importing test block");
            },
            |service, _| {
                let amount = 5 * CENTS;
                let to: Address = AccountPublic::from(bob.public()).into_account().into();
                let from: Address = AccountPublic::from(charlie.public()).into_account().into();
                let genesis_hash = service.client().block_hash(0).unwrap().unwrap();
                let best_hash = service.client().chain_info().best_hash;
                let (spec_version, transaction_version) = {
                    let version = service.client().runtime_version_at(best_hash).unwrap();
                    (version.spec_version, version.transaction_version)
                };
                let signer = charlie.clone();

                let function = RuntimeCall::Balances(BalancesCall::transfer {
                    dest: to.into(),
                    value: amount,
                });

                let check_non_zero_sender = frame_system::CheckNonZeroSender::new();
                let check_spec_version = frame_system::CheckSpecVersion::new();
                let check_tx_version = frame_system::CheckTxVersion::new();
                let check_genesis = frame_system::CheckGenesis::new();
                let check_era = frame_system::CheckEra::from(Era::Immortal);
                let check_nonce = frame_system::CheckNonce::from(index);
                let check_weight = frame_system::CheckWeight::new();
                let tx_payment = pallet_asset_tx_payment::ChargeAssetTxPayment::from(0, None);
                let extra = (
                    check_non_zero_sender,
                    check_spec_version,
                    check_tx_version,
                    check_genesis,
                    check_era,
                    check_nonce,
                    check_weight,
                    tx_payment,
                );
                let raw_payload = SignedPayload::from_raw(
                    function,
                    extra,
                    ((), spec_version, transaction_version, genesis_hash, genesis_hash, (), (), ()),
                );
                let signature = raw_payload.using_encoded(|payload| signer.sign(payload));
                let (function, extra, _) = raw_payload.deconstruct();
                index += 1;
                UncheckedExtrinsic::new_signed(function, from.into(), signature.into(), extra)
                    .into()
            },
        );
    }

    #[test]
    #[ignore]
    fn test_consensus() {
        sp_tracing::try_init_simple();

        sc_service_test::consensus(
            crate::chain_spec::tests::integration_test_config_with_two_authorities(),
            |config| {
                let NewFullBase { task_manager, client, network, transaction_pool, .. } =
                    new_full_base(config, false, |_, _| ())?;
                Ok(sc_service_test::TestNetComponents::new(
                    task_manager,
                    client,
                    network,
                    transaction_pool,
                ))
            },
            vec!["//Alice".into(), "//Bob".into()],
        )
    }
}