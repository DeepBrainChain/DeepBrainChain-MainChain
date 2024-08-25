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
use dbc_support::traits::DLCMachineSlashInfoTrait;
use frame_support::{ensure, pallet_prelude::Weight};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_evm::GasWeightMapping;

pub struct DLCMachineSlashInfo<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    GetDlcMachineSlashedAt = "getDlcMachineSlashedAt(string)",
    GetDlcMachineSlashedReportId = "getDlcMachineSlashedReportId(string)",
    GetDlcMachineSlashedReporter = "getDlcMachineSlashedReporter(string)",
}

impl<T> Precompile for DLCMachineSlashInfo<T>
where
    T: pallet_evm::Config + maintain_committee::Config,
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
            Selector::GetDlcMachineSlashedAt => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String, // machine_id
                    ],
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

                let slash_at  = <maintain_committee::Pallet<T> as DLCMachineSlashInfoTrait>::get_dlc_machine_slashed_at(machine_id_str.clone().as_bytes().to_vec());

                log::debug!(
                    target: LOG_TARGET,
                    "get_dlc_machine_slashed_at: machine_id: {:?}",
                    machine_id,
                );

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(slash_at.into())]),
                })
            },

            Selector::GetDlcMachineSlashedReportId => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String, // machine_id
                    ],
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

                let report_id  = <maintain_committee::Pallet<T> as DLCMachineSlashInfoTrait>::get_dlc_machine_slashed_report_id(machine_id_str.clone().as_bytes().to_vec());

                log::debug!(target: LOG_TARGET, " machine_id : {:?}", machine_id_str);

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(report_id.into())]),
                })
            },
            Selector::GetDlcMachineSlashedReporter => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String, // machine_id
                    ],
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

                let reporter  = <maintain_committee::Pallet<T> as DLCMachineSlashInfoTrait>::get_dlc_machine_slashed_reporter(machine_id_str.clone().as_bytes().to_vec());

                log::debug!(
                    target: LOG_TARGET,
                    "get_dlc_machine_slashed_reporter: machine_id: {:?}",
                    machine_id,
                );

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Address(reporter.into())]),
                })
            },
        }
    }
}
