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
use dbc_support::traits::ProjectRegister;
use frame_support::{ensure, pallet_prelude::Weight};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_evm::GasWeightMapping;
use sp_runtime::traits::SaturatedConversion;
use sp_std::vec::Vec;

pub struct AIProjectRegister<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    GetMachineCalcPoint = "getMachineCalcPoint(string)",
    MachineIsRegistered = "machineIsRegistered(string,string)",
    GetRentDuration = "getRentDuration(string,string,string,uint256,uint128[])",
    AddMachineRegisteredProject =
        "add_machine_registered_project(string,string,string,uint256,string,string)",
    RemovalMachineRegisteredProject =
        "remove_machine_registered_project(string,string,string,string,string)",
}

impl<T> Precompile for AIProjectRegister<T>
where
    T: pallet_evm::Config + pallet_balances::Config + ai_project_register::Config,
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

                let calc_point: U256 =
                    <ai_project_register::Pallet<T> as ProjectRegister>::get_machine_calc_point(
                        machine_id.clone(),
                    )
                    .into();

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
                let param = ethabi::decode(
                    &[ethabi::ParamType::String, ethabi::ParamType::String],
                    &input[4..],
                )
                .map_err(|e| PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: format!("decode param failed: {:?}", e).into(),
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

                let is_registered: bool =
                    <ai_project_register::Pallet<T> as ProjectRegister>::is_registered(
                        machine_id.clone(),
                        project_name.clone(),
                    );

                log::debug!(
                    target: LOG_TARGET,
                    ":machine_id: {:?}, project_name: {:?}, is_registered: {:?}",
                    machine_id,
                    project_name,
                    is_registered
                );

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Bool(is_registered)]),
                })
            },

            Selector::GetRentDuration => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String,    // msg
                        ethabi::ParamType::String,    // sig
                        ethabi::ParamType::String,    // public
                        ethabi::ParamType::String,    // stake_at
                        ethabi::ParamType::Uint(256), // machine_id
                        ethabi::ParamType::Bytes,     // rent_ids
                    ],
                    &input[4..],
                )
                .map_err(|e| PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: format!("decode param failed: {:?}", e).into(),
                })?;

                let msg =
                    param[0].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[0] failed".into(),
                    })?;

                let sig_str =
                    param[1].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                ensure!(
                    sig_str.as_bytes().len() == 64,
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "signature invalid input".into(),
                    }
                );

                let mut b = [0u8; 64];
                b.copy_from_slice(&sig_str.as_bytes()[0..64]);
                let sig = sp_core::sr25519::Signature(b);

                let public_str =
                    param[2].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[2] failed".into(),
                    })?;

                ensure!(
                    public_str.as_bytes().len() == 32,
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "public invalid input".into(),
                    }
                );

                let mut b = [0u8; 32];
                b.copy_from_slice(&sig_str.as_bytes()[0..32]);
                let public = sp_core::sr25519::Public(b);

                let machine_id_str =
                    param[3].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[3] failed".into(),
                    })?;

                let stake_at_block_number_uint =
                    param[4].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[4] failed".into(),
                    })?;

                let stake_at_block_number: u64 =
                    stake_at_block_number_uint.try_into().map_err(|e| {
                        PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: format!(
                                "take_at_block_number: {:?} to u64 failed: {:?}",
                                stake_at_block_number_uint, e
                            )
                            .into(),
                        }
                    })?;

                let rent_ids =
                    param[5].clone().into_bytes().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[5] failed".into(),
                    })?;
                let rent_ids: Vec<u64> = rent_ids.iter().map(|&x| x.into()).collect();
                let rent_ids_size = rent_ids.len() as u64;

                let duration  =<ai_project_register::Pallet<T> as ProjectRegister>::get_machine_valid_stake_duration(msg.clone().into_bytes(),sig.clone(),public.clone(), T::BlockNumber::saturated_from(stake_at_block_number.clone()), machine_id_str.clone().as_bytes().to_vec(), rent_ids.clone()).map_err( |e| {
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: e.into(),
                    }
                })?;
                log::debug!(target: LOG_TARGET, "msg : {:?}, sig : {:?}, public : {:?},  stake_at : {:?}, machine_id : {:?}, rent_ids: {:?},  get_machine_valid_stake_duration: duration: {:?}",msg,sig,public,stake_at_block_number,machine_id_str,rent_ids,duration);

                let weight = Weight::default().saturating_add(
                    <T as frame_system::Config>::DbWeight::get().reads(rent_ids_size),
                );

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(
                        duration.saturated_into::<u64>().into(),
                    )]),
                })
            },

            Selector::AddMachineRegisteredProject => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String,
                        ethabi::ParamType::String,
                        ethabi::ParamType::String,
                        ethabi::ParamType::Uint(256),
                        ethabi::ParamType::String,
                        ethabi::ParamType::String,
                    ],
                    &input[4..],
                )
                .map_err(|e| PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: format!("decode param failed: {:?}", e).into(),
                })?;

                let msg =
                    param[0].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[0] failed".into(),
                    })?;

                let sig_str =
                    param[1].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                ensure!(
                    sig_str.as_bytes().len() == 64,
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "signature invalid input".into(),
                    }
                );

                let mut b = [0u8; 64];
                b.copy_from_slice(&sig_str.as_bytes()[0..64]);
                let sig = sp_core::sr25519::Signature(b);

                let public_str =
                    param[2].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[2] failed".into(),
                    })?;

                ensure!(
                    public_str.as_bytes().len() == 32,
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "public invalid input".into(),
                    }
                );

                let mut b = [0u8; 32];
                b.copy_from_slice(&sig_str.as_bytes()[0..32]);
                let public = sp_core::sr25519::Public(b);

                let rent_id_uint =
                    param[3].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[3] failed".into(),
                    })?;

                let machine_id_str =
                    param[4].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[4] failed".into(),
                    })?;

                let project_name_str =
                    param[5].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[5] failed".into(),
                    })?;
                let project_name = project_name_str.as_bytes().to_vec();
                <ai_project_register::Pallet<T> as ProjectRegister>::add_machine_registered_project(
                        msg.clone().into_bytes(),sig.clone(),public.clone(),rent_id_uint.as_u64(),machine_id_str.clone().as_bytes().to_vec(),project_name.clone(),
                    ).map_err(|e|{
                        PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: e.into(),
                        }
                    })?;

                log::debug!(
                    target: LOG_TARGET,
                    ":machine_id: {:?}, project_name: {:?}, registered",
                    machine_id_str,
                    project_name,
                );

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(2));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Bool(true)]),
                })
            },

            Selector::RemovalMachineRegisteredProject => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String,
                        ethabi::ParamType::String,
                        ethabi::ParamType::String,
                        ethabi::ParamType::String,
                        ethabi::ParamType::String,
                    ],
                    &input[4..],
                )
                .map_err(|e| PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: format!("decode param failed: {:?}", e).into(),
                })?;

                let msg =
                    param[0].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[0] failed".into(),
                    })?;

                let sig_str =
                    param[1].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                ensure!(
                    sig_str.as_bytes().len() == 64,
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "signature invalid input".into(),
                    }
                );

                let mut b = [0u8; 64];
                b.copy_from_slice(&sig_str.as_bytes()[0..64]);
                let sig = sp_core::sr25519::Signature(b);

                let public_str =
                    param[2].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[2] failed".into(),
                    })?;

                ensure!(
                    public_str.as_bytes().len() == 32,
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "public invalid input".into(),
                    }
                );

                let mut b = [0u8; 32];
                b.copy_from_slice(&sig_str.as_bytes()[0..32]);
                let public = sp_core::sr25519::Public(b);

                let machine_id_str =
                    param[3].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[3] failed".into(),
                    })?;

                let project_name_str =
                    param[4].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[4] failed".into(),
                    })?;
                let project_name = project_name_str.as_bytes().to_vec();

                let _ =<ai_project_register::Pallet<T> as ProjectRegister>::remove_machine_registered_project(
                        msg.clone().into_bytes(),sig.clone(),public.clone(),machine_id_str.clone().as_bytes().to_vec(),project_name.clone(),
                    ).map_err(|e|{
                        PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: e.into(),
                        }
                    });

                log::debug!(
                    target: LOG_TARGET,
                    ":machine_id: {:?}, project_name: {:?}, unregistered",
                    machine_id_str,
                    project_name,
                );

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().writes(2));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Bool(true)]),
                })
            },
        }
    }
}
