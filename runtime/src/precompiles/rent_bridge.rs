// [+30% 桥] RentBridge precompile（hash(2052) = 0x…0804）
// 仅 DeepLink RentDBC 合约地址（mod.rs 白名单）可调。EVM 租出/退租中国机器时，
// 通知主链 online-profile 标记该机被 DeepLink 租用，使其享受/取消原生挖矿 +30% 被租加成。
// ⚠️ 共识相关：动挖矿奖励分配，须经 DBC 团队评审 + 测试网验证后再上主网。
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
use frame_support::{ensure, pallet_prelude::Weight};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use pallet_evm::GasWeightMapping;

pub struct RentBridge<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    SetMachineRentedForDeepLink = "setMachineRentedForDeepLink(string,bool)",
}

impl<T> Precompile for RentBridge<T>
where
    T: pallet_evm::Config + online_profile::Config,
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
            Selector::SetMachineRentedForDeepLink => {
                let param = ethabi::decode(
                    &[ethabi::ParamType::String, ethabi::ParamType::Bool],
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
                let is_rented =
                    param[1].clone().into_bool().ok_or_else(|| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "decode param[1] failed".into(),
                    })?;

                let machine_id = machine_id_str.as_bytes().to_vec();

                // [审计修 M-1] 先扣 gas 再改 Substrate 状态：deeplink_set_rented 直接写 Substrate 存储(EVM revert
                //   不回滚)，若 record_cost 排在其后、gas 不足 → OOG revert 被 EVM try/catch 吞、但状态已变更
                //   → 链上事件报 false 而实际已生效，误导对账。按 precompile 标准模式先 record_cost 后变更。
                // update_snap_on_rent_changed 涉及多处 era 快照 + sys/stash 读写，weight 给足
                let weight = Weight::default()
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(8))
                    .saturating_add(<T as frame_system::Config>::DbWeight::get().writes(7));
                handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))?;

                online_profile::Pallet::<T>::deeplink_set_rented(machine_id.clone(), is_rented)
                    .map_err(|_| PrecompileFailure::Revert {
                        exit_status: ExitRevert::Reverted,
                        output: "deeplink_set_rented failed".into(),
                    })?;

                log::debug!(
                    target: LOG_TARGET,
                    "rent_bridge: machine_id: {:?}, is_rented: {:?}",
                    machine_id,
                    is_rented
                );

                Ok(PrecompileOutput {
                    exit_status: ExitSucceed::Returned,
                    output: Default::default(),
                })
            },
        }
    }
}
