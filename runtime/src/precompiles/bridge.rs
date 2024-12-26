use fp_evm::{
    ExitRevert, ExitSucceed, Precompile, PrecompileFailure, PrecompileHandle, PrecompileOutput,
    PrecompileResult,
};
use sp_core::{Get, U256};
use sp_runtime::RuntimeDebug;
extern crate alloc;
use crate::precompiles::LOG_TARGET;
use alloc::format;
use core::marker::PhantomData;
use dbc_primitives::AccountId;
use frame_support::{
    ensure,
    pallet_prelude::{IsType, Weight},
    traits::{Currency, ExistenceRequirement},
};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_evm::{AddressMapping, GasWeightMapping};

pub struct Bridge<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    Transfer = "transfer(address,string,uint256)",
}

type BalanceOf<T> = <T as pallet_balances::Config>::Balance;

impl<T> Precompile for Bridge<T>
where
    T: pallet_evm::Config + pallet_balances::Config,
    BalanceOf<T>: TryFrom<U256> + Into<U256>,
    T::AccountId: IsType<AccountId>,
{
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        let input = handle.input();

        ensure!(
            input.len() >= 4,
            PrecompileFailure::Revert {
                exit_status: ExitRevert::Reverted,
                output: "invalid input".into(),
            }
        );

        let selector = u32::from_be_bytes(input[..4].try_into().expect("checked. qed!"));
        let selector: Selector = selector.try_into().map_err(|e| PrecompileFailure::Revert {
            exit_status: ExitRevert::Reverted,
            output: format!("invalid selector: {:?}", e).into(),
        })?;

        match selector {
            Selector::Transfer => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::Address,
                        ethabi::ParamType::String,
                        ethabi::ParamType::Uint(256),
                    ],
                    &input.get(4..).unwrap_or_default(),
                )
                .map_err(|e| PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: format!("decode param failed: {:?}", e).into(),
                })?;

                let from =
                    param[0].clone().into_address().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[0] failed".into(),
                    })?;
                let from: T::AccountId = T::AddressMapping::into_account_id(from);

                let to =
                    param[1].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                let to = to.strip_prefix("0x").unwrap_or(&to);
                ensure!(
                    to.len() == 64,
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("invalid address: {:?}", to.len()).into(),
                    }
                );

                let to_hex: [u8; 32] =
                    array_bytes::hex_n_into(to).map_err(|e| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("invalid address: {:?}", e).into(),
                    })?;
                let to: T::AccountId = T::AccountId::from(to_hex.into());

                let origin_amount =
                    param[2].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                // evm decimals is 18, native balance decimals is 15
                let amount = origin_amount.checked_div(U256::from(1000)).expect("checked. qed!");
                // check suffix is 000
                ensure!(
                    origin_amount == amount.saturating_mul(U256::from(1000)),
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("invalid amount, origin amount: {:?}", origin_amount)
                            .into(),
                    }
                );

                let amount: BalanceOf<T> =
                    amount.try_into().map_err(|_| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("invalid amount: {:?}", amount).into(),
                    })?;

                log::debug!(
                    target: LOG_TARGET,
                    "bridge: from: {:?}, to: {:?}, amount: {:?}",
                    from,
                    to,
                    amount
                );

                <pallet_balances::Pallet<T> as Currency<T::AccountId>>::transfer(
                    &from,
                    &to,
                    amount,
                    ExistenceRequirement::AllowDeath,
                )
                .map_err(|e| PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: format!("transfer failed: {:?}", e).into(),
                })?;

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(2))
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().writes(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: Default::default(),
                })
            },
        }
    }
}
