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

//! Substrate chain configurations.

use coins_bip32::ecdsa::{SigningKey, VerifyingKey};
use coins_bip39::{English, Mnemonic, Wordlist};
use dbc_runtime::{
    constants::currency::*, opaque::SessionKeys, wasm_binary_unwrap, AuthorityDiscoveryConfig,
    BabeConfig, BalancesConfig, BaseFeeConfig, Block, CouncilConfig, DefaultBaseFeePerGas,
    DefaultElasticity, DemocracyConfig, EVMChainIdConfig, EVMConfig, ElectionsConfig,
    GrandpaConfig, ImOnlineConfig, IndicesConfig, MaxNominations, NominationPoolsConfig,
    SessionConfig, StakerStatus, StakingConfig, SudoConfig, SystemConfig, TechnicalCommitteeConfig,
};
use fp_evm::GenesisAccount;
use k256::{elliptic_curve::sec1::ToEncodedPoint, EncodedPoint};
use pallet_im_online::sr25519::AuthorityId as ImOnlineId;
use sc_chain_spec::ChainSpecExtension;
use sc_service::ChainType;
use sc_telemetry::TelemetryEndpoints;
use serde::{Deserialize, Serialize};
use sp_authority_discovery::AuthorityId as AuthorityDiscoveryId;
use sp_consensus_babe::AuthorityId as BabeId;
use sp_consensus_grandpa::AuthorityId as GrandpaId;
use sp_core::{crypto::UncheckedInto, sr25519, Pair, Public, H160};
use sp_io::hashing::keccak_256;
use sp_runtime::{
    traits::{IdentifyAccount, Verify},
    Perbill,
};
use std::str::FromStr;

pub use dbc_primitives::{AccountId, Balance, Signature};
pub use dbc_runtime::GenesisConfig;

type AccountPublic = <Signature as Verify>::Signer;

const STAGING_TELEMETRY_URL: &str = "wss://telemetry.polkadot.io/submit/";
const DEFAULT_PROPS: &str = r#"
    {
        "tokenDecimals": 15,
        "tokenSymbol": "DBC"
    }
"#;

/// Node `ChainSpec` extensions.
///
/// Additional parameters for some Substrate core modules,
/// customizable from the chain spec.
#[derive(Default, Clone, Serialize, Deserialize, ChainSpecExtension)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
    /// Block numbers with known hashes.
    pub fork_blocks: sc_client_api::ForkBlocks<Block>,
    /// Known bad block hashes.
    pub bad_blocks: sc_client_api::BadBlocks<Block>,
    /// The light sync state extension used by the sync-state rpc.
    pub light_sync_state: sc_sync_state_rpc::LightSyncStateExtension,
}

/// Specialized `ChainSpec`.
pub type ChainSpec = sc_service::GenericChainSpec<GenesisConfig, Extensions>;

/// DBC Mainnet spec config
pub fn mainnet_config() -> Result<ChainSpec, String> {
    ChainSpec::from_json_bytes(&include_bytes!("../res/dbc-spec-v4.json")[..])

    // generate_mainnet_config
    // Ok(generate_mainnet_config())
}

fn session_keys(
    grandpa: GrandpaId,
    babe: BabeId,
    im_online: ImOnlineId,
    authority_discovery: AuthorityDiscoveryId,
) -> SessionKeys {
    SessionKeys { grandpa, babe, im_online, authority_discovery }
}

fn staging_testnet_config_genesis() -> GenesisConfig {
    #[rustfmt::skip]
	// stash, controller, session-key
	// generated with secret:
	// for i in 1 2 3 4 ; do for j in stash controller; do subkey inspect "$secret"/fir/$j/$i; done; done
	//
	// and
	//
	// for i in 1 2 3 4 ; do for j in session; do subkey --ed25519 inspect "$secret"//fir//$j//$i; done; done

	let initial_authorities: Vec<(
		AccountId,
		AccountId,
		GrandpaId,
		BabeId,
		ImOnlineId,
		AuthorityDiscoveryId,
	)> = vec![
		(
			// 5HEeqb1BG4MbEqgjWhEADcachLFxyymX6kCRefA2ou5CJETy
			array_bytes::hex_n_into_unchecked("e4c375aa31c6ef7c6d16fce4bad2cb5415e794ad813c4aff6e02cd1205b12b06"),
			// 5HEeqb1BG4MbEqgjWhEADcachLFxyymX6kCRefA2ou5CJETy
			array_bytes::hex_n_into_unchecked("e4c375aa31c6ef7c6d16fce4bad2cb5415e794ad813c4aff6e02cd1205b12b06"),
			// 5Gp6iEgoXC9TTVKEexuZuDn8czrDLQNTUXKtFCzTBTpV2Ghe
			array_bytes::hex2array_unchecked("d2096bf55f3c5d3df37525cef5bd7de93a8013d1fa9fa771ab3900a428a36dc8")
				.unchecked_into(),
			// 5HEeqb1BG4MbEqgjWhEADcachLFxyymX6kCRefA2ou5CJETy
			array_bytes::hex2array_unchecked("e4c375aa31c6ef7c6d16fce4bad2cb5415e794ad813c4aff6e02cd1205b12b06")
				.unchecked_into(),
			// 5HEeqb1BG4MbEqgjWhEADcachLFxyymX6kCRefA2ou5CJETy
			array_bytes::hex2array_unchecked("e4c375aa31c6ef7c6d16fce4bad2cb5415e794ad813c4aff6e02cd1205b12b06")
				.unchecked_into(),
			// 5HEeqb1BG4MbEqgjWhEADcachLFxyymX6kCRefA2ou5CJETy
			array_bytes::hex2array_unchecked("e4c375aa31c6ef7c6d16fce4bad2cb5415e794ad813c4aff6e02cd1205b12b06")
				.unchecked_into(),
		),
		(
			// 5Dq2mjgaM9eSUmMZGu3ArYjLj7DUyr5irZ19rLgYqvVqeGsx
			array_bytes::hex_n_into_unchecked("4e0ab466bfd7f78c4bd7930b0fd9c840f0f9d55169fcdb1eec336e7166343033"),
			// 5Dq2mjgaM9eSUmMZGu3ArYjLj7DUyr5irZ19rLgYqvVqeGsx
			array_bytes::hex_n_into_unchecked("4e0ab466bfd7f78c4bd7930b0fd9c840f0f9d55169fcdb1eec336e7166343033"),
			// 5DPbnyUcazqLRY5U1p9VrM1Eqh8sZx8nQQRNui5sVcNRcYXX
			array_bytes::hex2array_unchecked("3aa57be27e4f53363fb2884bc24a90796f020a956b71030032ccf698ea0dcd69")
				.unchecked_into(),
			// 5Dq2mjgaM9eSUmMZGu3ArYjLj7DUyr5irZ19rLgYqvVqeGsx
			array_bytes::hex2array_unchecked("4e0ab466bfd7f78c4bd7930b0fd9c840f0f9d55169fcdb1eec336e7166343033")
				.unchecked_into(),
			// 5Dq2mjgaM9eSUmMZGu3ArYjLj7DUyr5irZ19rLgYqvVqeGsx
			array_bytes::hex2array_unchecked("4e0ab466bfd7f78c4bd7930b0fd9c840f0f9d55169fcdb1eec336e7166343033")
				.unchecked_into(),
			// 5Dq2mjgaM9eSUmMZGu3ArYjLj7DUyr5irZ19rLgYqvVqeGsx
			array_bytes::hex2array_unchecked("4e0ab466bfd7f78c4bd7930b0fd9c840f0f9d55169fcdb1eec336e7166343033")
				.unchecked_into(),
		),
		(
			// 5DG4KYvuvfgUYt4b2CfVRNprZpFDW3jkELUnKZgYzt5xmrvx
			array_bytes::hex_n_into_unchecked("34e4d1b453c02158162e52a530e55ab254a3b60480b517dd224986b2d9b97508"),
			// 5DG4KYvuvfgUYt4b2CfVRNprZpFDW3jkELUnKZgYzt5xmrvx
			array_bytes::hex_n_into_unchecked("34e4d1b453c02158162e52a530e55ab254a3b60480b517dd224986b2d9b97508"),
			// 5HfHDMQWYPjjXJcS31sPtEAmdFSvrWCwiyPTaEg1kgFeYH8h
			array_bytes::hex2array_unchecked("f78bcca58bb837ae03106d5bb7b9b0c71e10673458d350cc3de74902e9ec6b15")
				.unchecked_into(),
			// 5DG4KYvuvfgUYt4b2CfVRNprZpFDW3jkELUnKZgYzt5xmrvx
			array_bytes::hex2array_unchecked("34e4d1b453c02158162e52a530e55ab254a3b60480b517dd224986b2d9b97508")
				.unchecked_into(),
			// 5DG4KYvuvfgUYt4b2CfVRNprZpFDW3jkELUnKZgYzt5xmrvx
			array_bytes::hex2array_unchecked("34e4d1b453c02158162e52a530e55ab254a3b60480b517dd224986b2d9b97508")
				.unchecked_into(),
			// 5DG4KYvuvfgUYt4b2CfVRNprZpFDW3jkELUnKZgYzt5xmrvx
			array_bytes::hex2array_unchecked("34e4d1b453c02158162e52a530e55ab254a3b60480b517dd224986b2d9b97508")
				.unchecked_into(),
		),
		(
			// 5FEmxL86rj2av2X1p7bVvLWZx7CSdFDUmhmWMF1EjUeoB9wg
			array_bytes::hex_n_into_unchecked("8c62f2bddcf1aaf61668b3225657ac075ba82595066ba3243b899ff6b695443d"),
			// 5FEmxL86rj2av2X1p7bVvLWZx7CSdFDUmhmWMF1EjUeoB9wg
			array_bytes::hex_n_into_unchecked("8c62f2bddcf1aaf61668b3225657ac075ba82595066ba3243b899ff6b695443d"),
			// 5DTnCRbvg6XjP9NQZ1HP7au6MB4MryBQaJ6bXcWL5WLwf2ES
			array_bytes::hex2array_unchecked("3dd57d565c0cb2cd33e9b46bdac6fc7bd8ff2b9ab7566929ddead33d92d48bc9")
				.unchecked_into(),
			// 5FEmxL86rj2av2X1p7bVvLWZx7CSdFDUmhmWMF1EjUeoB9wg
			array_bytes::hex2array_unchecked("8c62f2bddcf1aaf61668b3225657ac075ba82595066ba3243b899ff6b695443d")
				.unchecked_into(),
			// 5FEmxL86rj2av2X1p7bVvLWZx7CSdFDUmhmWMF1EjUeoB9wg
			array_bytes::hex2array_unchecked("8c62f2bddcf1aaf61668b3225657ac075ba82595066ba3243b899ff6b695443d")
				.unchecked_into(),
			// 5FEmxL86rj2av2X1p7bVvLWZx7CSdFDUmhmWMF1EjUeoB9wg
			array_bytes::hex2array_unchecked("8c62f2bddcf1aaf61668b3225657ac075ba82595066ba3243b899ff6b695443d")
				.unchecked_into(),
		),
	];

    // generated with secret: subkey inspect "$secret"/fir
    let root_key: AccountId = array_bytes::hex_n_into_unchecked(
        // 5DMESR6Zr58qvqtoFEritA8kDULLgCw5XRapffgUvSYSdfJN
        "38d71b8cec5c8d06d3bdc60f1d235612b8544acfe7f26bc0e3bdda12a3996a04",
    );

    let endowed_accounts: Vec<AccountId> = vec![root_key.clone()];

    testnet_genesis(initial_authorities, vec![], root_key, Some(endowed_accounts))
}

/// Staging testnet config.
pub fn staging_testnet_config() -> ChainSpec {
    let boot_nodes = vec![];
    ChainSpec::from_genesis(
        "DBC Testnet 2024",
        "staging_testnet_2024",
        ChainType::Live,
        staging_testnet_config_genesis,
        boot_nodes,
        Some(
            TelemetryEndpoints::new(vec![(STAGING_TELEMETRY_URL.to_string(), 0)])
                .expect("Staging telemetry url is valid; qed"),
        ),
        None,
        None,
        Some(serde_json::from_str(DEFAULT_PROPS).unwrap()),
        Default::default(),
    )
}

/// Helper function to generate a crypto pair from seed
pub fn get_from_seed<TPublic: Public>(seed: &str) -> <TPublic::Pair as Pair>::Public {
    TPublic::Pair::from_string(&format!("//{}", seed), None)
        .expect("static values are valid; qed")
        .public()
}

/// Helper function to generate an account ID from seed
pub fn get_account_id_from_seed<TPublic: Public>(seed: &str) -> AccountId
where
    AccountPublic: From<<TPublic::Pair as Pair>::Public>,
{
    AccountPublic::from(get_from_seed::<TPublic>(seed)).into_account()
}

/// Helper function to generate stash, controller and session key from seed
pub fn authority_keys_from_seed(
    seed: &str,
) -> (AccountId, AccountId, GrandpaId, BabeId, ImOnlineId, AuthorityDiscoveryId) {
    (
        get_account_id_from_seed::<sr25519::Public>(&format!("{}//stash", seed)),
        get_account_id_from_seed::<sr25519::Public>(seed),
        get_from_seed::<GrandpaId>(seed),
        get_from_seed::<BabeId>(seed),
        get_from_seed::<ImOnlineId>(seed),
        get_from_seed::<AuthorityDiscoveryId>(seed),
    )
}

/// Helper function to create GenesisConfig for testing
pub fn dev_testnet_genesis(
    initial_authorities: Vec<(
        AccountId,
        AccountId,
        GrandpaId,
        BabeId,
        ImOnlineId,
        AuthorityDiscoveryId,
    )>,
    initial_nominators: Vec<AccountId>,
    root_key: AccountId,
    endowed_accounts: Option<Vec<AccountId>>,
) -> GenesisConfig {
    let mut endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(|| {
        vec![
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            get_account_id_from_seed::<sr25519::Public>("Bob"),
            get_account_id_from_seed::<sr25519::Public>("Charlie"),
            get_account_id_from_seed::<sr25519::Public>("Dave"),
            get_account_id_from_seed::<sr25519::Public>("Eve"),
            get_account_id_from_seed::<sr25519::Public>("Ferdie"),
            get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
            get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
            get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
            get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
            get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
            get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
        ]
    });
    // endow all authorities and nominators.
    initial_authorities
        .iter()
        .map(|x| &x.0)
        .chain(initial_nominators.iter())
        .for_each(|x| {
            if !endowed_accounts.contains(x) {
                endowed_accounts.push(x.clone())
            }
        });

    // stakers: all validators and nominators.
    let mut rng = rand::thread_rng();
    let stakers = initial_authorities
        .iter()
        .map(|x| (x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator))
        .chain(initial_nominators.iter().map(|x| {
            use rand::{seq::SliceRandom, Rng};
            let limit = (MaxNominations::get() as usize).min(initial_authorities.len());
            let count = rng.gen::<usize>() % limit;
            let nominations = initial_authorities
                .as_slice()
                .choose_multiple(&mut rng, count)
                .into_iter()
                .map(|choice| choice.0.clone())
                .collect::<Vec<_>>();
            (x.clone(), x.clone(), STASH, StakerStatus::Nominator(nominations))
        }))
        .collect::<Vec<_>>();

    let num_endowed_accounts = endowed_accounts.len();

    const ENDOWMENT: Balance = 10_000_000 * DOLLARS;
    const STASH: Balance = ENDOWMENT / 1000;

    // We prefund the standard dev accounts
    let evm_accounts = get_evm_accounts(None);

    GenesisConfig {
        system: SystemConfig { code: wasm_binary_unwrap().to_vec() },
        balances: BalancesConfig {
            balances: endowed_accounts.iter().cloned().map(|x| (x, ENDOWMENT)).collect(),
        },
        indices: IndicesConfig { indices: vec![] },
        session: SessionConfig {
            keys: initial_authorities
                .iter()
                .map(|x| {
                    (
                        x.0.clone(),
                        x.0.clone(),
                        session_keys(x.2.clone(), x.3.clone(), x.4.clone(), x.5.clone()),
                    )
                })
                .collect::<Vec<_>>(),
        },
        staking: StakingConfig {
            validator_count: initial_authorities.len() as u32,
            minimum_validator_count: initial_authorities.len() as u32,
            invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
            slash_reward_fraction: Perbill::from_percent(10),
            stakers,
            ..Default::default()
        },
        democracy: DemocracyConfig::default(),
        elections: ElectionsConfig {
            members: endowed_accounts
                .iter()
                .take((num_endowed_accounts + 1) / 2)
                .cloned()
                .map(|member| (member, STASH))
                .collect(),
        },
        council: CouncilConfig::default(),
        technical_committee: TechnicalCommitteeConfig {
            members: endowed_accounts
                .iter()
                .take((num_endowed_accounts + 1) / 2)
                .cloned()
                .collect(),
            phantom: Default::default(),
        },
        sudo: SudoConfig { key: Some(root_key) },
        babe: BabeConfig {
            authorities: vec![],
            epoch_config: Some(dbc_runtime::BABE_GENESIS_EPOCH_CONFIG),
        },
        im_online: ImOnlineConfig { keys: vec![] },
        authority_discovery: AuthorityDiscoveryConfig { keys: vec![] },
        grandpa: GrandpaConfig { authorities: vec![] },
        treasury: Default::default(),
        // vesting: Default::default(),
        assets: pallet_assets::GenesisConfig {
            // This asset is used by the NIS pallet as counterpart currency.
            assets: vec![(9, get_account_id_from_seed::<sr25519::Public>("Alice"), true, 1)],
            ..Default::default()
        },
        // transaction_storage: Default::default(),
        transaction_payment: Default::default(),
        // alliance: Default::default(),
        // alliance_motion: Default::default(),
        nomination_pools: NominationPoolsConfig {
            min_create_bond: 10 * DOLLARS,
            min_join_bond: 1 * DOLLARS,
            ..Default::default()
        },
        ethereum: Default::default(),
        evm: EVMConfig {
            accounts: evm_accounts
                .iter()
                .map(|addr| {
                    (
                        (*addr).into(),
                        GenesisAccount {
                            nonce: Default::default(),
                            balance: (100_000 * DOLLARS).into(), // 1000 DBC
                            storage: Default::default(),
                            code: Default::default(),
                        },
                    )
                })
                .collect(),
        },
        evm_chain_id: EVMChainIdConfig { chain_id: 19850818u64 },
        base_fee: BaseFeeConfig::new(DefaultBaseFeePerGas::get(), DefaultElasticity::get()),
    }
}

/// Helper function to create GenesisConfig for testing
pub fn testnet_genesis(
    initial_authorities: Vec<(
        AccountId,
        AccountId,
        GrandpaId,
        BabeId,
        ImOnlineId,
        AuthorityDiscoveryId,
    )>,
    initial_nominators: Vec<AccountId>,
    root_key: AccountId,
    endowed_accounts: Option<Vec<AccountId>>,
) -> GenesisConfig {
    let mut endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_else(|| {
        vec![
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            get_account_id_from_seed::<sr25519::Public>("Bob"),
            get_account_id_from_seed::<sr25519::Public>("Charlie"),
            get_account_id_from_seed::<sr25519::Public>("Dave"),
            get_account_id_from_seed::<sr25519::Public>("Eve"),
            get_account_id_from_seed::<sr25519::Public>("Ferdie"),
            get_account_id_from_seed::<sr25519::Public>("Alice//stash"),
            get_account_id_from_seed::<sr25519::Public>("Bob//stash"),
            get_account_id_from_seed::<sr25519::Public>("Charlie//stash"),
            get_account_id_from_seed::<sr25519::Public>("Dave//stash"),
            get_account_id_from_seed::<sr25519::Public>("Eve//stash"),
            get_account_id_from_seed::<sr25519::Public>("Ferdie//stash"),
        ]
    });
    // endow all authorities and nominators.
    initial_authorities
        .iter()
        .map(|x| &x.0)
        .chain(initial_nominators.iter())
        .for_each(|x| {
            if !endowed_accounts.contains(x) {
                endowed_accounts.push(x.clone())
            }
        });

    // stakers: all validators and nominators.
    let mut rng = rand::thread_rng();
    let stakers = initial_authorities
        .iter()
        .map(|x| (x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator))
        .chain(initial_nominators.iter().map(|x| {
            use rand::{seq::SliceRandom, Rng};
            let limit = (MaxNominations::get() as usize).min(initial_authorities.len());
            let count = rng.gen::<usize>() % limit;
            let nominations = initial_authorities
                .as_slice()
                .choose_multiple(&mut rng, count)
                .into_iter()
                .map(|choice| choice.0.clone())
                .collect::<Vec<_>>();
            (x.clone(), x.clone(), STASH, StakerStatus::Nominator(nominations))
        }))
        .collect::<Vec<_>>();

    let num_endowed_accounts = endowed_accounts.len();

    const ENDOWMENT: Balance = 10_000_000 * DOLLARS;
    const STASH: Balance = ENDOWMENT / 1000;

    GenesisConfig {
        system: SystemConfig { code: wasm_binary_unwrap().to_vec() },
        balances: BalancesConfig {
            balances: endowed_accounts.iter().cloned().map(|x| (x, ENDOWMENT)).collect(),
        },
        indices: IndicesConfig { indices: vec![] },
        session: SessionConfig {
            keys: initial_authorities
                .iter()
                .map(|x| {
                    (
                        x.0.clone(),
                        x.0.clone(),
                        session_keys(x.2.clone(), x.3.clone(), x.4.clone(), x.5.clone()),
                    )
                })
                .collect::<Vec<_>>(),
        },
        staking: StakingConfig {
            validator_count: initial_authorities.len() as u32,
            minimum_validator_count: initial_authorities.len() as u32,
            invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
            slash_reward_fraction: Perbill::from_percent(10),
            stakers,
            ..Default::default()
        },
        democracy: DemocracyConfig::default(),
        elections: ElectionsConfig {
            members: endowed_accounts
                .iter()
                .take((num_endowed_accounts + 1) / 2)
                .cloned()
                .map(|member| (member, STASH))
                .collect(),
        },
        council: CouncilConfig::default(),
        technical_committee: TechnicalCommitteeConfig {
            members: endowed_accounts
                .iter()
                .take((num_endowed_accounts + 1) / 2)
                .cloned()
                .collect(),
            phantom: Default::default(),
        },
        sudo: SudoConfig { key: Some(root_key) },
        babe: BabeConfig {
            authorities: vec![],
            epoch_config: Some(dbc_runtime::BABE_GENESIS_EPOCH_CONFIG),
        },
        im_online: ImOnlineConfig { keys: vec![] },
        authority_discovery: AuthorityDiscoveryConfig { keys: vec![] },
        grandpa: GrandpaConfig { authorities: vec![] },
        treasury: Default::default(),
        // vesting: Default::default(),
        assets: Default::default(),
        // transaction_storage: Default::default(),
        transaction_payment: Default::default(),
        // alliance: Default::default(),
        // alliance_motion: Default::default(),
        nomination_pools: NominationPoolsConfig {
            min_create_bond: 10 * DOLLARS,
            min_join_bond: 1 * DOLLARS,
            ..Default::default()
        },
        ethereum: Default::default(),
        evm: Default::default(),
        evm_chain_id: EVMChainIdConfig { chain_id: 19850818u64 },
        base_fee: BaseFeeConfig::new(DefaultBaseFeePerGas::get(), DefaultElasticity::get()),
    }
}

/// Mainnet config
#[allow(dead_code)]
fn generate_mainnet_config() -> ChainSpec {
    ChainSpec::from_genesis(
        "DBC Mainnet",
        "dbc_network_mainnet",
        ChainType::Live,
        mainnet_config_genesis,
        vec![], // boot_nodes
        Some(
            TelemetryEndpoints::new(vec![(STAGING_TELEMETRY_URL.to_string(), 0)])
                .expect("Staging telemetry url is valid; qed"),
        ),
        None,
        None,
        Some(serde_json::from_str(DEFAULT_PROPS).unwrap()),
        Default::default(),
    )
}

fn mainnet_config_genesis() -> GenesisConfig {
    let initial_authorities: Vec<(
        AccountId,
        AccountId,
        GrandpaId,
        BabeId,
        ImOnlineId,
        AuthorityDiscoveryId,
    )> = vec![
        (
            // 5HEeqb1BG4MbEqgjWhEADcachLFxyymX6kCRefA2ou5CJETy
            array_bytes::hex_n_into_unchecked(
                "e4c375aa31c6ef7c6d16fce4bad2cb5415e794ad813c4aff6e02cd1205b12b06",
            ),
            // 5HEeqb1BG4MbEqgjWhEADcachLFxyymX6kCRefA2ou5CJETy
            array_bytes::hex_n_into_unchecked(
                "e4c375aa31c6ef7c6d16fce4bad2cb5415e794ad813c4aff6e02cd1205b12b06",
            ),
            // 5DTnCRbvg6XjP9NQZ1HP7au6MB4MryBQaJ6bXcWL5WLwf2ES
            array_bytes::hex2array_unchecked(
                "3dd57d565c0cb2cd33e9b46bdac6fc7bd8ff2b9ab7566929ddead33d92d48bc9",
            )
            .unchecked_into(),
            // 5HEeqb1BG4MbEqgjWhEADcachLFxyymX6kCRefA2ou5CJETy
            array_bytes::hex2array_unchecked(
                "e4c375aa31c6ef7c6d16fce4bad2cb5415e794ad813c4aff6e02cd1205b12b06",
            )
            .unchecked_into(),
            // 5HEeqb1BG4MbEqgjWhEADcachLFxyymX6kCRefA2ou5CJETy
            array_bytes::hex2array_unchecked(
                "e4c375aa31c6ef7c6d16fce4bad2cb5415e794ad813c4aff6e02cd1205b12b06",
            )
            .unchecked_into(),
            // 5HEeqb1BG4MbEqgjWhEADcachLFxyymX6kCRefA2ou5CJETy
            array_bytes::hex2array_unchecked(
                "e4c375aa31c6ef7c6d16fce4bad2cb5415e794ad813c4aff6e02cd1205b12b06",
            )
            .unchecked_into(),
        ),
        (
            // 5Dq2mjgaM9eSUmMZGu3ArYjLj7DUyr5irZ19rLgYqvVqeGsx
            array_bytes::hex_n_into_unchecked(
                "4e0ab466bfd7f78c4bd7930b0fd9c840f0f9d55169fcdb1eec336e7166343033",
            ),
            // 5Dq2mjgaM9eSUmMZGu3ArYjLj7DUyr5irZ19rLgYqvVqeGsx
            array_bytes::hex_n_into_unchecked(
                "4e0ab466bfd7f78c4bd7930b0fd9c840f0f9d55169fcdb1eec336e7166343033",
            ),
            // 5DPbnyUcazqLRY5U1p9VrM1Eqh8sZx8nQQRNui5sVcNRcYXX
            array_bytes::hex2array_unchecked(
                "3aa57be27e4f53363fb2884bc24a90796f020a956b71030032ccf698ea0dcd69",
            )
            .unchecked_into(),
            // 5Dq2mjgaM9eSUmMZGu3ArYjLj7DUyr5irZ19rLgYqvVqeGsx
            array_bytes::hex2array_unchecked(
                "4e0ab466bfd7f78c4bd7930b0fd9c840f0f9d55169fcdb1eec336e7166343033",
            )
            .unchecked_into(),
            // 5Dq2mjgaM9eSUmMZGu3ArYjLj7DUyr5irZ19rLgYqvVqeGsx
            array_bytes::hex2array_unchecked(
                "4e0ab466bfd7f78c4bd7930b0fd9c840f0f9d55169fcdb1eec336e7166343033",
            )
            .unchecked_into(),
            // 5Dq2mjgaM9eSUmMZGu3ArYjLj7DUyr5irZ19rLgYqvVqeGsx
            array_bytes::hex2array_unchecked(
                "4e0ab466bfd7f78c4bd7930b0fd9c840f0f9d55169fcdb1eec336e7166343033",
            )
            .unchecked_into(),
        ),
        (
            // 5DG4KYvuvfgUYt4b2CfVRNprZpFDW3jkELUnKZgYzt5xmrvx
            array_bytes::hex_n_into_unchecked(
                "34e4d1b453c02158162e52a530e55ab254a3b60480b517dd224986b2d9b97508",
            ),
            // 5DG4KYvuvfgUYt4b2CfVRNprZpFDW3jkELUnKZgYzt5xmrvx
            array_bytes::hex_n_into_unchecked(
                "34e4d1b453c02158162e52a530e55ab254a3b60480b517dd224986b2d9b97508",
            ),
            // 5Gp6iEgoXC9TTVKEexuZuDn8czrDLQNTUXKtFCzTBTpV2Ghe
            array_bytes::hex2array_unchecked(
                "d2096bf55f3c5d3df37525cef5bd7de93a8013d1fa9fa771ab3900a428a36dc8",
            )
            .unchecked_into(),
            // 5DG4KYvuvfgUYt4b2CfVRNprZpFDW3jkELUnKZgYzt5xmrvx
            array_bytes::hex2array_unchecked(
                "34e4d1b453c02158162e52a530e55ab254a3b60480b517dd224986b2d9b97508",
            )
            .unchecked_into(),
            // 5DG4KYvuvfgUYt4b2CfVRNprZpFDW3jkELUnKZgYzt5xmrvx
            array_bytes::hex2array_unchecked(
                "34e4d1b453c02158162e52a530e55ab254a3b60480b517dd224986b2d9b97508",
            )
            .unchecked_into(),
            // 5DG4KYvuvfgUYt4b2CfVRNprZpFDW3jkELUnKZgYzt5xmrvx
            array_bytes::hex2array_unchecked(
                "34e4d1b453c02158162e52a530e55ab254a3b60480b517dd224986b2d9b97508",
            )
            .unchecked_into(),
        ),
        (
            // 5FEmxL86rj2av2X1p7bVvLWZx7CSdFDUmhmWMF1EjUeoB9wg
            array_bytes::hex_n_into_unchecked(
                "8c62f2bddcf1aaf61668b3225657ac075ba82595066ba3243b899ff6b695443d",
            ),
            // 5FEmxL86rj2av2X1p7bVvLWZx7CSdFDUmhmWMF1EjUeoB9wg
            array_bytes::hex_n_into_unchecked(
                "8c62f2bddcf1aaf61668b3225657ac075ba82595066ba3243b899ff6b695443d",
            ),
            // 5HfHDMQWYPjjXJcS31sPtEAmdFSvrWCwiyPTaEg1kgFeYH8h
            array_bytes::hex2array_unchecked(
                "f78bcca58bb837ae03106d5bb7b9b0c71e10673458d350cc3de74902e9ec6b15",
            )
            .unchecked_into(),
            // 5FEmxL86rj2av2X1p7bVvLWZx7CSdFDUmhmWMF1EjUeoB9wg
            array_bytes::hex2array_unchecked(
                "8c62f2bddcf1aaf61668b3225657ac075ba82595066ba3243b899ff6b695443d",
            )
            .unchecked_into(),
            // 5FEmxL86rj2av2X1p7bVvLWZx7CSdFDUmhmWMF1EjUeoB9wg
            array_bytes::hex2array_unchecked(
                "8c62f2bddcf1aaf61668b3225657ac075ba82595066ba3243b899ff6b695443d",
            )
            .unchecked_into(),
            // 5FEmxL86rj2av2X1p7bVvLWZx7CSdFDUmhmWMF1EjUeoB9wg
            array_bytes::hex2array_unchecked(
                "8c62f2bddcf1aaf61668b3225657ac075ba82595066ba3243b899ff6b695443d",
            )
            .unchecked_into(),
        ),
    ];

    // generated with secret: subkey inspect "$secret"/fir
    let root_key: AccountId = array_bytes::hex_n_into_unchecked(
        // 5G4Tx92gWuzdfWRqhX5UyywmPn4Zj1VR4mJARmywo2cD2KkU
        "b0c21f849124c82d6ebcd1ed2cce15dba356ea6670c13e6a7bf3815fae1ce53b",
    );

    let endowed_accounts: Vec<AccountId> = vec![root_key.clone()];

    mainnet_genesis(initial_authorities, vec![], root_key, Some(endowed_accounts))
}

/// Helper function to create GenesisConfig for mainnet
pub fn mainnet_genesis(
    initial_authorities: Vec<(
        AccountId,
        AccountId,
        GrandpaId,
        BabeId,
        ImOnlineId,
        AuthorityDiscoveryId,
    )>,
    initial_nominators: Vec<AccountId>,
    root_key: AccountId,
    endowed_accounts: Option<Vec<AccountId>>,
) -> GenesisConfig {
    let mut endowed_accounts: Vec<AccountId> = endowed_accounts.unwrap_or_default();
    // endow all authorities and nominators.
    initial_authorities
        .iter()
        .map(|x| &x.0)
        .chain(initial_nominators.iter())
        .for_each(|x| {
            if !endowed_accounts.contains(x) {
                endowed_accounts.push(x.clone())
            }
        });

    // stakers: all validators and nominators.
    let mut rng = rand::thread_rng();
    let stakers = initial_authorities
        .iter()
        .map(|x| (x.0.clone(), x.1.clone(), STASH, StakerStatus::Validator))
        .chain(initial_nominators.iter().map(|x| {
            use rand::{seq::SliceRandom, Rng};
            let limit = (MaxNominations::get() as usize).min(initial_authorities.len());
            let count = rng.gen::<usize>() % limit;
            let nominations = initial_authorities
                .as_slice()
                .choose_multiple(&mut rng, count)
                .into_iter()
                .map(|choice| choice.0.clone())
                .collect::<Vec<_>>();
            (x.clone(), x.clone(), STASH, StakerStatus::Nominator(nominations))
        }))
        .collect::<Vec<_>>();

    let num_endowed_accounts = endowed_accounts.len();

    const ENDOWMENT: Balance = 10_000_000 * DOLLARS;
    const STASH: Balance = ENDOWMENT / 1000;

    GenesisConfig {
        system: SystemConfig { code: wasm_binary_unwrap().to_vec() },
        balances: BalancesConfig {
            balances: endowed_accounts.iter().cloned().map(|x| (x, ENDOWMENT)).collect(),
        },
        indices: IndicesConfig { indices: vec![] },
        session: SessionConfig {
            keys: initial_authorities
                .iter()
                .map(|x| {
                    (
                        x.0.clone(),
                        x.0.clone(),
                        session_keys(x.2.clone(), x.3.clone(), x.4.clone(), x.5.clone()),
                    )
                })
                .collect::<Vec<_>>(),
        },
        staking: StakingConfig {
            validator_count: initial_authorities.len() as u32,
            minimum_validator_count: initial_authorities.len() as u32,
            invulnerables: initial_authorities.iter().map(|x| x.0.clone()).collect(),
            slash_reward_fraction: Perbill::from_percent(10),
            stakers,
            ..Default::default()
        },
        democracy: DemocracyConfig::default(),
        elections: ElectionsConfig {
            members: endowed_accounts
                .iter()
                .take((num_endowed_accounts + 1) / 2)
                .cloned()
                .map(|member| (member, STASH))
                .collect(),
        },
        council: CouncilConfig::default(),
        technical_committee: TechnicalCommitteeConfig {
            members: endowed_accounts
                .iter()
                .take((num_endowed_accounts + 1) / 2)
                .cloned()
                .collect(),
            phantom: Default::default(),
        },
        sudo: SudoConfig { key: Some(root_key) },
        babe: BabeConfig {
            authorities: vec![],
            epoch_config: Some(dbc_runtime::BABE_GENESIS_EPOCH_CONFIG),
        },
        im_online: ImOnlineConfig { keys: vec![] },
        authority_discovery: AuthorityDiscoveryConfig { keys: vec![] },
        grandpa: GrandpaConfig { authorities: vec![] },
        treasury: Default::default(),
        // vesting: Default::default(),
        assets: Default::default(),
        // transaction_storage: Default::default(),
        transaction_payment: Default::default(),
        // alliance: Default::default(),
        // alliance_motion: Default::default(),
        nomination_pools: NominationPoolsConfig {
            min_create_bond: 10 * DOLLARS,
            min_join_bond: 1 * DOLLARS,
            ..Default::default()
        },
        ethereum: Default::default(),
        evm: Default::default(),
        evm_chain_id: EVMChainIdConfig { chain_id: 19880818u64 },
        base_fee: BaseFeeConfig::new(DefaultBaseFeePerGas::get(), DefaultElasticity::get()),
    }
}

fn development_config_genesis() -> GenesisConfig {
    dev_testnet_genesis(
        vec![authority_keys_from_seed("Alice")],
        vec![],
        get_account_id_from_seed::<sr25519::Public>("Alice"),
        None,
    )
}

/// Development config (single validator Alice)
pub fn development_config() -> ChainSpec {
    ChainSpec::from_genesis(
        "Development",
        "dev",
        ChainType::Development,
        development_config_genesis,
        vec![],
        None,
        None,
        None,
        Some(serde_json::from_str(DEFAULT_PROPS).unwrap()),
        Default::default(),
    )
}

fn local_testnet_genesis() -> GenesisConfig {
    testnet_genesis(
        vec![authority_keys_from_seed("Alice"), authority_keys_from_seed("Bob")],
        vec![],
        get_account_id_from_seed::<sr25519::Public>("Alice"),
        None,
    )
}

/// Local testnet config (multivalidator Alice + Bob)
pub fn local_testnet_config() -> ChainSpec {
    ChainSpec::from_genesis(
        "Local Testnet",
        "local_testnet",
        ChainType::Local,
        local_testnet_genesis,
        vec![],
        None,
        None,
        None,
        Some(serde_json::from_str(DEFAULT_PROPS).unwrap()),
        Default::default(),
    )
}

fn generate_evm_address<W: Wordlist>(phrase: &str, index: u32) -> H160 {
    let derivation_path =
        coins_bip32::path::DerivationPath::from_str(&format!("m/44'/60'/0'/0/{}", index))
            .expect("should parse the default derivation path");
    let mnemonic = Mnemonic::<W>::new_from_phrase(phrase).unwrap();

    let derived_priv_key = mnemonic.derive_key(derivation_path, None).unwrap();
    let key: &SigningKey = derived_priv_key.as_ref();
    let secret_key: SigningKey = SigningKey::from_bytes(&key.to_bytes()).unwrap();
    let verify_key: VerifyingKey = secret_key.verifying_key();

    let point: &EncodedPoint = &verify_key.to_encoded_point(false);
    let public_key = point.to_bytes();

    let hash = keccak_256(&public_key[1..]);
    let address = H160::from_slice(&hash[12..]);

    log::info!(
        "private_key: 0x{:?} --------> Address: {:x?}",
        sp_core::hexdisplay::HexDisplay::from(&key.to_bytes().to_vec()),
        address
    );
    address
}

fn get_evm_accounts(mnemonic: Option<&str>) -> Vec<H160> {
    let phrase =
        mnemonic.unwrap_or("bottom drive obey lake curtain smoke basket hold race lonely fit walk");
    let mut evm_accounts = Vec::new();
    for index in 0..10u32 {
        let addr = generate_evm_address::<English>(phrase, index);
        evm_accounts.push(addr);
    }
    evm_accounts
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::service::{new_full_base, NewFullBase};
    use sp_runtime::BuildStorage;

    fn local_testnet_genesis_instant_single() -> GenesisConfig {
        testnet_genesis(
            vec![authority_keys_from_seed("Alice")],
            vec![],
            get_account_id_from_seed::<sr25519::Public>("Alice"),
            None,
        )
    }

    /// Local testnet config (single validator - Alice)
    pub fn integration_test_config_with_single_authority() -> ChainSpec {
        ChainSpec::from_genesis(
            "Integration Test",
            "test",
            ChainType::Development,
            local_testnet_genesis_instant_single,
            vec![],
            None,
            None,
            None,
            None,
            Default::default(),
        )
    }

    /// Local testnet config (multivalidator Alice + Bob)
    pub fn integration_test_config_with_two_authorities() -> ChainSpec {
        ChainSpec::from_genesis(
            "Integration Test",
            "test",
            ChainType::Development,
            local_testnet_genesis,
            vec![],
            None,
            None,
            None,
            None,
            Default::default(),
        )
    }

    #[test]
    fn test_create_development_chain_spec() {
        development_config().build_storage().unwrap();
    }

    #[test]
    fn test_create_local_testnet_chain_spec() {
        local_testnet_config().build_storage().unwrap();
    }

    #[test]
    fn test_staging_test_net_chain_spec() {
        staging_testnet_config().build_storage().unwrap();
    }
}
