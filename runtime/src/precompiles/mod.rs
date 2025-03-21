use fp_evm::{ExitRevert, PrecompileFailure};
use pallet_evm::{
    IsPrecompileResult, Precompile, PrecompileHandle, PrecompileResult, PrecompileSet,
};
use scale_info::prelude::format;
use sp_core::H160;
use sp_std::marker::PhantomData;

use pallet_evm_precompile_blake2::Blake2F;
use pallet_evm_precompile_bn128::{Bn128Add, Bn128Mul, Bn128Pairing};
use pallet_evm_precompile_dispatch::Dispatch;
use pallet_evm_precompile_modexp::Modexp;
use pallet_evm_precompile_sha3fips::Sha3FIPS256;
use pallet_evm_precompile_simple::{ECRecover, ECRecoverPublicKey, Identity, Ripemd160, Sha256};

mod bridge;
use bridge::Bridge;
mod dbc_price;
use dbc_price::DBCPrice;

mod dlc_price;
mod machine_info;
use dlc_price::DLCPrice;

use machine_info::MachineInfo;
const LOG_TARGET: &str = "evm";

pub struct DBCPrecompiles<T>(PhantomData<T>);

impl<T> DBCPrecompiles<T>
where
    T: pallet_evm::Config,
{
    pub fn new() -> Self {
        Self(Default::default())
    }
    pub fn used_addresses() -> [H160; 11] {
        [
            hash(1),
            hash(2),
            hash(3),
            hash(4),
            hash(5),
            hash(1024),
            hash(1025),
            hash(1026),
            hash(2048),
            hash(2049),
            hash(2051),
        ]
    }
}
impl<T> PrecompileSet for DBCPrecompiles<T>
where
    T: pallet_evm::Config + eth_precompile_whitelist::Config,
    Dispatch<T>: Precompile,
    Bridge<T>: Precompile,
    DBCPrice<T>: Precompile,
    MachineInfo<T>: Precompile,
    DLCPrice<T>: Precompile,
{
    fn execute(&self, handle: &mut impl PrecompileHandle) -> Option<PrecompileResult> {
        let address = handle.code_address();
        let context = handle.context();
        log::debug!(target: LOG_TARGET, "PrecompileSet execute address: {:?}, context: {:?}", address, handle.context());

        if let IsPrecompileResult::Answer { is_precompile: true, extra_cost: _ } =
            self.is_precompile(address, handle.remaining_gas())
        {
            if address > hash(9) && context.address != address {
                return Some(Err(PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: "cannot be called with DELEGATECALL or CALLCODE".into(),
                }))
            }

            // check if the context.caller in the precompile whitelist
            let precompile_whitelist =
                eth_precompile_whitelist::PrecompileWhitelist::<T>::get(address);

            match address {
                a if a == hash(2048) => {
                    if !precompile_whitelist.contains(&context.caller) {
                        log::debug!(target: LOG_TARGET, "caller {:?} not in the {:?} whitelist", context.caller, address);

                        return Some(Err(PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: format!("caller {:?} not in the whitelist", context.caller)
                                .into(),
                        }))
                    }
                },
                _ => {},
            }
        }

        match address {
            // Ethereum precompiles :
            a if a == hash(1) => Some(ECRecover::execute(handle)),
            a if a == hash(2) => Some(Sha256::execute(handle)),
            a if a == hash(3) => Some(Ripemd160::execute(handle)),
            a if a == hash(4) => Some(Identity::execute(handle)),
            a if a == hash(5) => Some(Modexp::execute(handle)),
            a if a == hash(6) => Some(Bn128Add::execute(handle)),
            a if a == hash(7) => Some(Bn128Mul::execute(handle)),
            a if a == hash(8) => Some(Bn128Pairing::execute(handle)),
            a if a == hash(9) => Some(Blake2F::execute(handle)),
            // Non-Frontier specific nor Ethereum precompiles :
            a if a == hash(1024) => Some(Sha3FIPS256::execute(handle)),
            a if a == hash(1025) => Some(Dispatch::<T>::execute(handle)),
            a if a == hash(1026) => Some(ECRecoverPublicKey::execute(handle)),

            // DBC specific precompiles
            a if a == hash(2048) => Some(Bridge::<T>::execute(handle)),
            a if a == hash(2049) => Some(DBCPrice::<T>::execute(handle)),
            a if a == hash(2051) => Some(MachineInfo::<T>::execute(handle)),
            a if a == hash(2050) => Some(DLCPrice::<T>::execute(handle)),

            _ => None,
        }
    }

    fn is_precompile(&self, address: H160, _gas: u64) -> IsPrecompileResult {
        IsPrecompileResult::Answer {
            is_precompile: Self::used_addresses().contains(&address),
            extra_cost: 0,
        }
    }
}

fn hash(a: u64) -> H160 {
    H160::from_low_u64_be(a)
}
