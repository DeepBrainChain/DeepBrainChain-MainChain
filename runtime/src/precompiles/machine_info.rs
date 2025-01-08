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
use dbc_support::traits::MachineInfoTrait;
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
    GetMachineCPURate = "getMachineCPURate(string)",
    GetMachineGPUCount = "getMachineGPUCount(string)",
    GetRentEndAt = "getRentEndAt(string,uint256)",
    IsMachineOwner = "isMachineOwner(string,address)",
    GetDLCMachineRentFee = "getDLCMachineRentFee(string,uint256,uint256)",
    GetDBCMachineRentFee = "getDBCMachineRentFee(string,uint256,uint256)",
    GetUSDTMachineRentFee = "getUSDTMachineRentFee(string,uint256,uint256)",
    GetDLCRentFeeByCalcPoint = "getDLCRentFeeByCalcPoint(uint256,uint256,uint256,uint256)",
}

impl<T> Precompile for MachineInfo<T>
where
    T: pallet_evm::Config + pallet_balances::Config + rent_machine::Config,
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
                let param = ethabi::decode(
                    &[ethabi::ParamType::String],
                    &input.get(4..).unwrap_or_default(),
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

            Selector::GetMachineCPURate => {
                let param = ethabi::decode(
                    &[ethabi::ParamType::String],
                    &input.get(4..).unwrap_or_default(),
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

                let cpu_rate: U256 =
                    <rent_machine::Pallet<T> as MachineInfoTrait>::get_machine_cpu_rate(
                        machine_id.clone(),
                    )
                    .into();

                log::debug!(
                    target: LOG_TARGET,
                    "get_machine_cpu_rate: machine_id: {:?}, cpu_rate: {:?}",
                    machine_id,
                    cpu_rate
                );

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(cpu_rate)]),
                })
            },

            Selector::GetMachineGPUCount => {
                let param = ethabi::decode(
                    &[ethabi::ParamType::String],
                    &input.get(4..).unwrap_or_default(),
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

                let gpu_num: U256 =
                    <rent_machine::Pallet<T> as MachineInfoTrait>::get_machine_gpu_num(
                        machine_id.clone(),
                    )
                    .into();

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(gpu_num)]),
                })
            },

            Selector::GetRentEndAt => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String,    // machine_id
                        ethabi::ParamType::Uint(256), // rent_id
                    ],
                    &input.get(4..).unwrap_or_default(),
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
                let rent_id_uint =
                    param[1].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;
                let rent_id: u64 = rent_id_uint.as_u64();
                let end_at = <rent_machine::Pallet<T> as MachineInfoTrait>::get_rent_end_at(
                    machine_id_str.clone().as_bytes().to_vec(),
                    rent_id,
                )
                .map_err(|e| PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: format!(
                        "err: {}, machine_id: {}, rent_id: {}",
                        e, machine_id_str, rent_id
                    )
                    .into(),
                })?;

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(
                        end_at.saturated_into::<u64>().into(),
                    )]),
                })
            },

            Selector::IsMachineOwner => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String,  // machine_id
                        ethabi::ParamType::Address, // evm_address
                    ],
                    &input.get(4..).unwrap_or_default(),
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
                let machine_id = machine_id_str.clone().as_bytes().to_vec();
                let evm_address =
                    param[1].clone().into_address().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                let is_owner = <rent_machine::Pallet<T> as MachineInfoTrait>::is_machine_owner(
                    machine_id.clone(),
                    evm_address,
                )
                .map_err(|e| PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: format!(
                        "err: {}, machine_id: {}, evm_address: {}",
                        e, machine_id_str, evm_address
                    )
                    .into(),
                })?;

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Bool(is_owner)]),
                })
            },

            Selector::GetDLCMachineRentFee => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String,    // machine_id
                        ethabi::ParamType::Uint(256), // rent_block_numbers
                        ethabi::ParamType::Uint(8),   // rent_gpu_count
                    ],
                    &input.get(4..).unwrap_or_default(),
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
                let machine_id = machine_id_str.clone().as_bytes().to_vec();

                let rent_duration_uint =
                    param[1].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                let rent_duration: u64 = rent_duration_uint.as_u64();

                let rent_gpu_count_uint =
                    param[2].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[2] failed".into(),
                    })?;

                let rent_gpu_count: u32 = rent_gpu_count_uint.as_u32();

                let rent_fee =
                    <rent_machine::Pallet<T> as MachineInfoTrait>::get_dlc_machine_rent_fee(
                        machine_id.clone(),
                        rent_duration.saturated_into(),
                        rent_gpu_count,
                    )
                    .map_err(|e| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!(
                            " err: {}, machine_id: {}, rent_duration: {}, rent_gpu_count: {}",
                            e, machine_id_str, rent_duration, rent_gpu_count
                        )
                        .into(),
                    })?;

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(rent_fee.into())]),
                })
            },

            Selector::GetDBCMachineRentFee => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String,    // machine_id
                        ethabi::ParamType::Uint(256), // rent_block_numbers
                        ethabi::ParamType::Uint(8),   // rent_gpu_count
                    ],
                    &input.get(4..).unwrap_or_default(),
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
                let machine_id = machine_id_str.clone().as_bytes().to_vec();

                let rent_duration_uint =
                    param[1].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                let rent_duration: u64 = rent_duration_uint.as_u64();

                let rent_gpu_count_uint =
                    param[2].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[2] failed".into(),
                    })?;

                let rent_gpu_count: u32 = rent_gpu_count_uint.as_u32();

                let rent_fee =
                    <rent_machine::Pallet<T> as MachineInfoTrait>::get_dbc_machine_rent_fee(
                        machine_id.clone(),
                        rent_duration.saturated_into(),
                        rent_gpu_count,
                    )
                    .map_err(|e| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!(
                            " err: {}, machine_id: {}, rent_duration: {}, rent_gpu_count: {}",
                            e, machine_id_str, rent_duration, rent_gpu_count
                        )
                        .into(),
                    })?;

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(rent_fee.into())]),
                })
            },

            Selector::GetUSDTMachineRentFee => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String,    // machine_id
                        ethabi::ParamType::Uint(256), // rent_block_numbers
                        ethabi::ParamType::Uint(8),   // rent_gpu_count
                    ],
                    &input.get(4..).unwrap_or_default(),
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
                let machine_id = machine_id_str.clone().as_bytes().to_vec();

                let rent_duration_uint =
                    param[1].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                let rent_duration: u64 = rent_duration_uint.as_u64();

                let rent_gpu_count_uint =
                    param[2].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[2] failed".into(),
                    })?;

                let rent_gpu_count: u32 = rent_gpu_count_uint.as_u32();

                let rent_fee =
                    <rent_machine::Pallet<T> as MachineInfoTrait>::get_usdt_machine_rent_fee(
                        machine_id.clone(),
                        rent_duration.saturated_into(),
                        rent_gpu_count,
                    )
                    .map_err(|e| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!(
                            " err: {}, machine_id: {}, rent_duration: {}, rent_gpu_count: {}",
                            e, machine_id_str, rent_duration, rent_gpu_count
                        )
                        .into(),
                    })?;

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(rent_fee.into())]),
                })
            },
            Selector::GetDLCRentFeeByCalcPoint => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::Uint(256), // calc_point
                        ethabi::ParamType::Uint(256), // rent_block_numbers
                        ethabi::ParamType::Uint(8),   // rent_gpu_count
                        ethabi::ParamType::Uint(8),   // total_gpu_count
                    ],
                    &input.get(4..).unwrap_or_default(),
                )
                .map_err(|e| PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: format!("decode param failed: {:?}", e).into(),
                })?;

                let calc_point_uint =
                    param[0].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[0] failed".into(),
                    })?;

                let calc_point: u64 = calc_point_uint.as_u64();

                let rent_duration_uint =
                    param[1].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                let rent_duration: u64 = rent_duration_uint.as_u64();

                let rent_gpu_count_uint =
                    param[2].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[2] failed".into(),
                    })?;

                let rent_gpu_count: u32 = rent_gpu_count_uint.as_u32();

                let total_gpu_count_uint =
                    param[3].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[3] failed".into(),
                    })?;

                let total_gpu_count: u32 = total_gpu_count_uint.as_u32();

                let rent_fee =
                    <rent_machine::Pallet<T> as MachineInfoTrait>::get_dlc_rent_fee_by_calc_point(
                        calc_point,
                        rent_duration.saturated_into(),
                        rent_gpu_count,
                        total_gpu_count,
                    )
                        .map_err(|e| PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: format!(
                                " err: {}, calc_point: {}, rent_duration: {}, rent_gpu_count: {},total_gpu_count: {}",
                                e, calc_point, rent_duration, rent_gpu_count,total_gpu_count
                            )
                                .into(),
                        })?;

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(rent_fee.into())]),
                })
            },
        }
    }
}
