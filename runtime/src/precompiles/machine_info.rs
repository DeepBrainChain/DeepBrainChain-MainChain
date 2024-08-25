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
use dbc_support::traits::{DLCMachineInfoTrait, MachineInfoTrait};
use frame_support::{ensure, pallet_prelude::Weight};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_evm::GasWeightMapping;
use sp_runtime::traits::SaturatedConversion;

pub struct MachineInfo<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    GetMachineCalcPoint = "getMachineCalcPoint(string)",
    GetRentDuration = "getRentDuration(string,string,string,uint256,uint256,string)",
    GetDlcMachineRentDuration = "getDlcMachineRentDuration(uint256,uint256,string)",
}

impl<T> Precompile for MachineInfo<T>
where
    T: pallet_evm::Config
        + pallet_balances::Config
        + rent_machine::Config
        + rent_dlc_machine::Config,
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
                    <rent_machine::Pallet<T> as MachineInfoTrait>::get_machine_calc_point(
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

            Selector::GetRentDuration => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String,    // msg
                        ethabi::ParamType::String,    // sig
                        ethabi::ParamType::String,    // public
                        ethabi::ParamType::Uint(256), // last_claim_at
                        ethabi::ParamType::Uint(256), // slash_at
                        ethabi::ParamType::String,    // machine_id
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

                let last_claim_at_block_number_uint =
                    param[3].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[3] failed".into(),
                    })?;

                let last_claim_at_block_number: u64 = last_claim_at_block_number_uint.as_u64();

                let slash_at_block_number_uint =
                    param[4].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[4] failed".into(),
                    })?;

                let slash_at_block_number: u64 = slash_at_block_number_uint.as_u64();

                let machine_id_str =
                    param[5].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[5] failed".into(),
                    })?;

                let duration  = <rent_machine::Pallet<T> as MachineInfoTrait>::get_machine_valid_stake_duration(msg.clone().into_bytes(),sig.clone(),public.clone(), T::BlockNumber::saturated_from(last_claim_at_block_number.clone()),T::BlockNumber::saturated_from(slash_at_block_number.clone()), machine_id_str.clone().as_bytes().to_vec()).map_err( |e| {
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("err: {}, msg: {}, sig: {:?}, public: {:?}, last_claim_at: {},slash_at: {}, machine_id: {}",e,msg,sig,public,last_claim_at_block_number,slash_at_block_number,machine_id_str).into(),
                    }
                })?;
                log::debug!(target: LOG_TARGET, "msg : {:?}, sig : {:?}, public : {:?}, last_claim_at: {},slash_at: {}, machine_id : {:?},  get_machine_valid_stake_duration: duration: {:?}",msg,sig,public,last_claim_at_block_number,slash_at_block_number,machine_id_str,duration);

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(
                        duration.saturated_into::<u64>().into(),
                    )]),
                })
            },

            Selector::GetDlcMachineRentDuration => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::Uint(256), // last_claim_at
                        ethabi::ParamType::Uint(256), // slash_at
                        ethabi::ParamType::String,    // machine_id
                    ],
                    &input[4..],
                )
                .map_err(|e| PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: format!("decode param failed: {:?}", e).into(),
                })?;

                let last_claim_at_block_number_uint =
                    param[0].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[0] failed".into(),
                    })?;

                let last_claim_at_block_number: u64 = last_claim_at_block_number_uint.as_u64();

                let slash_at_block_number_uint =
                    param[1].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                let slash_at_block_number: u64 = slash_at_block_number_uint.as_u64();

                let machine_id_str =
                    param[2].clone().into_string().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[2] failed".into(),
                    })?;

                let duration  = <rent_dlc_machine::Pallet<T> as DLCMachineInfoTrait>::get_dlc_machine_rent_duration( T::BlockNumber::saturated_from(last_claim_at_block_number.clone()),T::BlockNumber::saturated_from(slash_at_block_number.clone()), machine_id_str.clone().as_bytes().to_vec()).map_err( |e| {
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("err: {},  last_claim_at: {},slash_at: {}, machine_id: {}",e,last_claim_at_block_number,slash_at_block_number,machine_id_str).into(),
                    }
                })?;
                log::debug!(target: LOG_TARGET, "last_claim_at: {},slash_at: {}, machine_id : {:?},  get_machine_valid_stake_duration: duration: {:?}",last_claim_at_block_number,slash_at_block_number,machine_id_str,duration);

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(
                        duration.saturated_into::<u64>().into(),
                    )]),
                })
            },
        }
    }
}