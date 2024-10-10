use fp_evm::{
    ExitRevert, ExitSucceed, Precompile, PrecompileFailure, PrecompileHandle, PrecompileOutput,
    PrecompileResult,
};
use sp_core::Get;
use sp_runtime::{RuntimeDebug, SaturatedConversion};
extern crate alloc;
use crate::precompiles::LOG_TARGET;
use alloc::format;
use core::marker::PhantomData;
use dbc_support::traits::DLCMachineReportStakingTrait;
use frame_support::{ensure, pallet_prelude::Weight};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_evm::GasWeightMapping;

pub struct DLCMachineReportStaking<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    ReportDlcStaking = "reportDlcStaking(string,string,string,string)",
    ReportDlcEndStaking = "reportDlcEndStaking(string,string,string,string)",

    ReportDlcNftStaking = "reportDlcNftStaking(string,string,string,string,uint256)",
    ReportDlcNftEndStaking = "reportDlcNftEndStaking(string,string,string,string,uint256)",
    GetValidRewardDuration = "getValidRewardDuration(uint256,uint256,uint256)",
    GetDlcNftStakingRewardStartAt = "getDlcNftStakingRewardStartAt(uint256)",
}

impl<T> Precompile for DLCMachineReportStaking<T>
where
    T: pallet_evm::Config + dlc_machine::Config,
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
            Selector::ReportDlcStaking => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String, // msg
                        ethabi::ParamType::String, // sig
                        ethabi::ParamType::String, // public
                        ethabi::ParamType::String, // machine_id
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

                let machine_id = machine_id_str.as_bytes().to_vec();

                let _ =
                    <dlc_machine::Pallet<T> as DLCMachineReportStakingTrait>::report_dlc_staking(
                        msg.clone().into_bytes(),
                        sig.clone(),
                        public.clone(),
                        machine_id_str.clone().as_bytes().to_vec(),
                    )
                    .map_err(|e| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!(
                            "err: {}, msg: {}, sig: {:?}, public: {:?}, machine_id: {}",
                            e, msg, sig, public, machine_id_str
                        )
                        .into(),
                    })?;

                log::debug!(target: LOG_TARGET, "report_dlc_staking: machine_id: {:?}", machine_id,);

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().writes(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Bool(true)]),
                })
            },

            Selector::ReportDlcEndStaking => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String, // msg
                        ethabi::ParamType::String, // sig
                        ethabi::ParamType::String, // public
                        ethabi::ParamType::String, // machine_id
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

                let _  = <dlc_machine::Pallet<T> as DLCMachineReportStakingTrait>::report_dlc_end_staking(msg.clone().into_bytes(),sig.clone(),public.clone(), machine_id_str.clone().as_bytes().to_vec()).map_err( |e| {
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("err: {}, msg: {}, sig: {:?}, public: {:?}, machine_id: {}",e,msg,sig,public, machine_id_str).into(),
                    }
                })?;
                log::debug!(
                    target: LOG_TARGET,
                    "msg : {:?}, sig : {:?}, public : {:?}, machine_id : {:?}",
                    msg,
                    sig,
                    public,
                    machine_id_str
                );

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().writes(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Bool(true)]),
                })
            },

            Selector::ReportDlcNftStaking => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String,    // msg
                        ethabi::ParamType::String,    // sig
                        ethabi::ParamType::String,    // public
                        ethabi::ParamType::String,    // machine_id
                        ethabi::ParamType::Uint(256), // phase_level
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

                let machine_id = machine_id_str.as_bytes().to_vec();

                let phase_level_uint =
                    param[4].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[4] failed".into(),
                    })?;
                let phase_level: u64 = phase_level_uint.as_u64();

                let _ =
                    <dlc_machine::Pallet<T> as DLCMachineReportStakingTrait>::report_dlc_nft_staking(
                        msg.clone().into_bytes(),
                        sig.clone(),
                        public.clone(),
                        machine_id_str.clone().as_bytes().to_vec(),
                        phase_level.into()
                    )
                        .map_err(|e| PrecompileFailure::Revert {
                            exit_status: ExitRevert::Reverted,
                            output: format!(
                                "err: {}, msg: {}, sig: {:?}, public: {:?}, machine_id: {}",
                                e, msg, sig, public, machine_id_str
                            )
                                .into(),
                        })?;

                log::debug!(target: LOG_TARGET, "report_dlc_nft_staking: machine_id: {:?}", machine_id,);

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().writes(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Bool(true)]),
                })
            },

            Selector::ReportDlcNftEndStaking => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::String,    // msg
                        ethabi::ParamType::String,    // sig
                        ethabi::ParamType::String,    // public
                        ethabi::ParamType::String,    // machine_id
                        ethabi::ParamType::Uint(256), // phase_level
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

                let phase_level_uint =
                    param[4].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[4] failed".into(),
                    })?;
                let phase_level: u64 = phase_level_uint.as_u64();

                let _  = <dlc_machine::Pallet<T> as DLCMachineReportStakingTrait>::report_dlc_nft_end_staking(msg.clone().into_bytes(),sig.clone(),public.clone(), machine_id_str.clone().as_bytes().to_vec(),phase_level.into()).map_err( |e| {
                    PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: format!("err: {}, msg: {}, sig: {:?}, public: {:?}, machine_id: {}",e,msg,sig,public, machine_id_str).into(),
                    }
                })?;
                log::debug!(
                    target: LOG_TARGET,
                    "msg : {:?}, sig : {:?}, public : {:?}, machine_id : {:?}",
                    msg,
                    sig,
                    public,
                    machine_id_str
                );

                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().writes(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Bool(true)]),
                })
            },

            Selector::GetValidRewardDuration => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::Uint(256), // last_claim_at
                        ethabi::ParamType::Uint(256), // total_stake_duration
                        ethabi::ParamType::Uint(256), // phase_level
                    ],
                    &input[4..],
                )
                .map_err(|e| PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: format!("decode param failed: {:?}", e).into(),
                })?;

                let last_claim_at_uint =
                    param[0].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[0] failed".into(),
                    })?;

                let total_stake_duration_uint =
                    param[1].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                let phase_level_uint =
                    param[2].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[2] failed".into(),
                    })?;
                let phase_level: u64 = phase_level_uint.as_u64();

                let last_claim_at: u64 = last_claim_at_uint.as_u64();
                let total_stake_duration: u64 = total_stake_duration_uint.as_u64();

                let valid_duration  = <dlc_machine::Pallet<T> as DLCMachineReportStakingTrait>::get_nft_staking_valid_reward_duration(T::BlockNumber::saturated_from(last_claim_at), T::BlockNumber::saturated_from(total_stake_duration), phase_level.into());
                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(
                        valid_duration.saturated_into::<u64>().into(),
                    )]),
                })
            },

            Selector::GetDlcNftStakingRewardStartAt => {
                let param = ethabi::decode(
                    &[
                        ethabi::ParamType::Uint(256), // phase_level
                    ],
                    &input[4..],
                )
                .map_err(|e| PrecompileFailure::Revert {
                    exit_status: ExitRevert::Reverted,
                    output: format!("decode param failed: {:?}", e).into(),
                })?;

                let phase_level_uint =
                    param[0].clone().into_uint().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[0] failed".into(),
                    })?;
                let phase_level: u64 = phase_level_uint.as_u64();

                let phase_one_reward_start_at  = <dlc_machine::Pallet<T> as DLCMachineReportStakingTrait>::get_nft_staking_reward_start_at(&phase_level.into());
                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(1));

                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: ethabi::encode(&[ethabi::Token::Uint(
                        phase_one_reward_start_at.saturated_into::<u64>().into(),
                    )]),
                })
            },
        }
    }
}
