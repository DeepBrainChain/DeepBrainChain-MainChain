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
use dbc_support::traits::{AiProjectRegister};
use frame_support::{ensure, pallet_prelude::Weight};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use sp_std::{vec::Vec};
use sp_runtime::traits::SaturatedConversion;
use pallet_evm::{AddressMapping, GasWeightMapping};




pub struct AIProjectRegister<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    GetMachineCalcPoint = "getMachineCalcPoint(string memory)",
    MachineIsRegistered = "machineIsRegistered(string memory)",
    GetRentDuration = "getRentDuration(string memory,uint256,uint128[])",
}

impl<T> Precompile for AIProjectRegister<T>
where
    T: pallet_evm::Config + pallet_balances::Config+ai_project_register::Config,
{
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult  {
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
            Selector::GetMachineCalcPoint => {
                let param =
                    ethabi::decode(&[ethabi::ParamType::String], &input[4..]).map_err(|e| {
                        PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: format!("decode param failed: {:?}", e).into(),
                        }
                    })?;

                let machine_id_str =
                    param[0].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[0] failed".into(),
                    })?;

                let machine_id = machine_id_str.as_bytes().to_vec();

                let calc_point :U256 = <ai_project_register::Pallet<T> as AiProjectRegister>::get_machine_calc_point(machine_id.clone()).into();

                log::debug!(
                    target: LOG_TARGET,
                    "get_machine_calc_point: machine_id: {:?}, calc_point: {:?}",
                    machine_id,
                    calc_point
                );

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(calc_point)]),
                })
            },
            Selector::MachineIsRegistered => {
                let param =
                    ethabi::decode(&[ethabi::ParamType::String], &input[4..]).map_err(|e| {
                        PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: format!("decode param failed: {:?}", e).into(),
                        }
                    })?;

                let machine_id_str =
                    param[0].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[0] failed".into(),
                    })?;

                let machine_id = machine_id_str.as_bytes().to_vec();

                let project_name_str =
                    param[1].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;
                let project_name = project_name_str.as_bytes().to_vec();

                let is_registered : bool = <ai_project_register::Pallet<T> as AiProjectRegister>::is_registered(machine_id,project_name);

                log::debug!(target: LOG_TARGET, ": is_registered: {:?}", is_registered);


                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Bool(is_registered)]),
                })
            },

            Selector::GetRentDuration => {
                let param =
                    ethabi::decode(&[ethabi::ParamType::Uint(256)], &input[4..]).map_err(|e| {
                        PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: format!("decode param failed: {:?}", e).into(),
                        }
                    })?;

                let stake_holder_address =
                    param[0].clone().into_address().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[0] failed".into(),
                    })?;

                let stake_holder = T::AddressMapping::into_account_id(stake_holder_address);

                let machine_id_str =
                    param[1].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                let stake_at_block_number_uint =
                    param[2].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                let stake_at_block_number: u64 =
                    stake_at_block_number_uint.try_into().map_err(|e| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("take_at_block_number: {:?} to u64 failed: {:?}", stake_at_block_number_uint, e).into(),
                    })?;

                let rent_ids: Vec<u8> =
                    param[3].clone().into_bytes().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[0] failed".into(),
                    })?;
                let rent_ids:Vec<u64> = rent_ids.iter().map(|&x| x.into()).collect();
                let rent_ids_size = rent_ids.len() as u64;
                let duration  =<ai_project_register::Pallet<T> as AiProjectRegister>::get_machine_valid_stake_duration(stake_holder, T::BlockNumber::saturated_from(stake_at_block_number),machine_id_str.as_bytes().to_vec(), rent_ids);

                log::debug!(target: LOG_TARGET, "get_machine_valid_stake_duration: duration: {:?}", duration);



                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(rent_ids_size));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(duration.saturated_into::<u64>().into())]),
                })
            },

        }
    }
}
