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
use dbc_support::traits::DbcPrice;
use frame_support::{ensure, pallet_prelude::Weight, traits::Currency};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_evm::GasWeightMapping;

pub struct DBCPrice<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    GetDBCPrice = "getDBCPrice()",
    GetDBCAmountByValue = "getDBCAmountByValue(uint256)",
}

type BalanceOf<T> = <<T as dbc_price_ocw::Config>::Currency as Currency<
    <T as frame_system::Config>::AccountId,
>>::Balance;

impl<T> Precompile for DBCPrice<T>
where
    T: pallet_evm::Config + pallet_balances::Config + dbc_price_ocw::Config,
    BalanceOf<T>: TryFrom<U256> + Into<U256>,
{
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        let input = handle.input();

        ensure!(
            input.len() > 4,
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
            Selector::GetDBCPrice => {
                let origin_value: U256 = <dbc_price_ocw::Pallet<T> as DbcPrice>::get_dbc_price()
                    .map(|v| v.into())
                    .unwrap_or_default();

                // evm decimals is 18, native balance decimals is 15
                let value = origin_value.saturating_mul(U256::from(1000));

                log::debug!(
                    target: LOG_TARGET,
                    "dbc-price: value: {:?}, origin value: {:?}",
                    value,
                    origin_value
                );

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(value)]),
                })
            },
            Selector::GetDBCAmountByValue => {
                let param =
                    ethabi::decode(&[ethabi::ParamType::Uint(256)], &input[4..]).map_err(|e| {
                        PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: format!("decode param failed: {:?}", e).into(),
                        }
                    })?;

                let origin_value =
                    param[0].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[0] failed".into(),
                    })?;

                let value: u64 =
                    origin_value.try_into().map_err(|e| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("value: {:?} to u64 failed: {:?}", origin_value, e).into(),
                    })?;

                log::debug!(target: LOG_TARGET, "dbc-price: value: {:?}", value);

                let amount: U256 =
                    <dbc_price_ocw::Pallet<T> as DbcPrice>::get_dbc_amount_by_value(value)
                        .map(|v| v.into())
                        .unwrap_or_default();

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(amount)]),
                })
            },
        }
    }
}
