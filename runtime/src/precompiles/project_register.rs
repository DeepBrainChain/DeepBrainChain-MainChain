use fp_evm::{
    ExitRevert, ExitSucceed, Precompile, PrecompileFailure, PrecompileHandle, PrecompileOutput,
    PrecompileResult,
};
use sp_core::Get;
use sp_runtime::RuntimeDebug;
extern crate alloc;
use crate::precompiles::LOG_TARGET;
use alloc::format;
use core::marker::PhantomData;
use dbc_support::traits::ProjectRegister;
use frame_support::{ensure, pallet_prelude::Weight};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_evm::GasWeightMapping;

pub struct AIProjectRegister<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    MachineIsRegistered = "machineIsRegistered(string,string)",
    AddMachineRegisteredProject = "addMachineRegisteredProject(string,string,string,string,string)",
    RemovalMachineRegisteredProject =
        "removeMachineRegisteredProject(string,string,string,string,string)",
    IsRegisteredMachineOwner = "isRegisteredMachineOwner(string,string,string,string,string)",
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
                    "machine_id: {:?}, project_name: {:?}, is_registered: {:?}",
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

            Selector::AddMachineRegisteredProject => {
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

                let sig =
                    hex::decode(sig_str.as_bytes()).map_err(|e| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("decode sig failed: {:?}", e).into(),
                    })?;

                let mut b = [0u8; 64];
                b.copy_from_slice(&sig[..]);
                let sig = sp_core::sr25519::Signature(b);

                let public_str =
                    param[2].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[2] failed".into(),
                    })?;

                let public =
                    hex::decode(public_str.as_bytes()).map_err(|e| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("decode pub key failed: {:?}", e).into(),
                    })?;

                let mut b = [0u8; 32];
                b.copy_from_slice(&public[..]);
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
                <ai_project_register::Pallet<T> as ProjectRegister>::add_machine_registered_project(
                        msg.clone().into_bytes(),sig.clone(),public.clone(),machine_id_str.clone().as_bytes().to_vec(),project_name.clone(),
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
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().writes(1));

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

                let sig =
                    hex::decode(sig_str.as_bytes()).map_err(|e| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("decode sig failed: {:?}", e).into(),
                    })?;

                let mut b = [0u8; 64];
                b.copy_from_slice(&sig[..]);
                let sig = sp_core::sr25519::Signature(b);

                let public_str =
                    param[2].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[2] failed".into(),
                    })?;

                let public =
                    hex::decode(public_str.as_bytes()).map_err(|e| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("decode pub key failed: {:?}", e).into(),
                    })?;

                let mut b = [0u8; 32];
                b.copy_from_slice(&public[..]);
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

                <ai_project_register::Pallet<T> as ProjectRegister>::remove_machine_registered_project(
                        msg.clone().into_bytes(),sig.clone(),public.clone(),machine_id_str.clone().as_bytes().to_vec(),project_name.clone(),
                    ).map_err(|e|{
                        PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: e.into(),
                        }
                    })?;

                log::debug!(
                    target: LOG_TARGET,
                    ":machine_id: {:?}, project_name: {:?}, unregistered",
                    machine_id_str,
                    project_name,
                );

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().writes(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Bool(true)]),
                })
            },

            Selector::IsRegisteredMachineOwner => {
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

                let sig =
                    hex::decode(sig_str.as_bytes()).map_err(|e| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("decode sig failed: {:?}", e).into(),
                    })?;

                let mut b = [0u8; 64];
                b.copy_from_slice(&sig[..]);
                let sig = sp_core::sr25519::Signature(b);

                let public_str =
                    param[2].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[2] failed".into(),
                    })?;

                let public =
                    hex::decode(public_str.as_bytes()).map_err(|e| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("decode pub key failed: {:?}", e).into(),
                    })?;

                let mut b = [0u8; 32];
                b.copy_from_slice(&public[..]);
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
                let is_owner = <ai_project_register::Pallet<T> as ProjectRegister>::is_registered_machine_owner(
                    msg.clone().into_bytes(),sig.clone(),public.clone(),machine_id_str.clone().as_bytes().to_vec(),project_name.clone(),
                ).map_err(|e|{
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: e.into(),
                    }
                })?;

                log::debug!(
                    target: LOG_TARGET,
                    "pub_key: {:?}, machine_id: {:?}, project_name: {:?}, is registered owner result :{}",
                    public_str,
                    machine_id_str,
                    project_name,
                    is_owner,

                );

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(2));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Bool(true)]),
                })
            },
        }
    }
}
