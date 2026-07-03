// ============================================================================
// MiningBridge precompile — DRAFT（DeepLink 提交，待 DBC 团队评审）  hash(2053) = 0x…0805
// ----------------------------------------------------------------------------
// 目的：让【矿工的 EVM 0x 钱包】直接驱动 online-profile 的 DBC 原生挖矿注册/质押/领奖/自救全流程，
//       使 DeepLink 桌面客户端能把中国网吧机器接入 DBC 挖矿，而无需矿工做 Substrate 钱包。
//
// dispatch 模型：以【调用者 0x → HashedAddressMapping 映射的 Substrate 账户】作为 stash==controller，
//       用 RawOrigin::Signed(该账户) 【直接调用 online_profile 的 extrinsic 函数】（非通用 0x1025 派发，
//       以便逐操作显式计 gas，避开占位假权重被绕的 DoS 面）。质押扣该 0x 钱包 DBC 余额、奖励回它。
//
// ★★ 与 rent_bridge 的区别：rent_bridge 是【白名单】(仅 RentDBC 合约可调，因为它给 +30% 加成)；
//     MiningBridge【对所有映射账户开放不加白名单】——矿工只能注册/操作自己映射账户名下的机器，
//     online_profile 自带 is_controller/machine_stash 越权保护，不凭空给任何东西。故 mod.rs 不把 2053 加入白名单分支。
//
// ⚠️ 共识/奖励相邻：与 rent_bridge 同规矩——DBC 团队评审 + 测试网跑通全流程 + 122.99.183.50 重编 WASM 后才上主网。
//
// ── v1 硬门槛（DBC 团队 2026-07-02 评审要求，已实现） ────────────────────────────
//  [G1] caller 必须是 EOA：经合约(AA/router/multicall)调 → 映射账户变成中间合约影子账户(无私钥不可达)，机器会绑到谁都控制不了的账户。
//  [G2] 全生命周期暴露：映射账户无私钥、只能经本 precompile 操作 → controller 一生会用到的操作必须全暴露，漏一个=遇到即永久卡死。
//       尤其 🔴 applySlashReview——被误罚(离线/假冒可达质押 50~100%)时 controller 唯一链上防御，漏=烧钱不可逆。
//  [G3] 直接 pallet-fn 调用 + 每 op 显式权重(era 快照类给足)。
//  [G4] OOG/revert 原子性：Frontier 不回滚 Substrate 存储 → 先 record_cost 再改状态；SCALE 解码失败干净 revert 不留残留。
//  [G5] setSelfController 幂等：底层 set_controller 二次调用会 AlreadyController revert → 已 set 过则跳过按成功返回。
//
// ── DBC 团队评审时请确认（标记 TODO(DBC)）──
//  - 各 op 的 record_cost 权重（下方为 DeepLink 估算，era 快照类需 DBC 按真实读写调准）。
//  - online_profile 的 storage getter / 类型路径（StakerCustomizeInfo / SlashId / MachineId / controller_stash getter）在本 crate 的确切引用名。
//  - EOA 判定用 pallet_evm::AccountCodes 是否合适。
//  - machineExit 有 1 年门槛 + 需 Online → 注册中途放弃/委员会拒的机器无 EVM 自救退质押口，DBC 是否补一个 pre-online abort+refund。
//  - set_controller 抢注 grief：攻击者可预算受害者映射 SS58 先占坑，setSelfController 是否需自愈/清理路径。
// ============================================================================

use fp_evm::{
    ExitError, ExitRevert, ExitSucceed, Precompile, PrecompileFailure, PrecompileHandle,
    PrecompileOutput, PrecompileResult,
};
use frame_support::pallet_prelude::Weight;
use frame_support::traits::Get; // [DBC 评审修] T::DbWeight::get() 需 Get 在作用域
use frame_system::RawOrigin;
use pallet_evm::{AddressMapping, GasWeightMapping};
use parity_scale_codec::Decode;
use sp_core::H256;
use sp_std::vec::Vec;
extern crate alloc;
use crate::precompiles::LOG_TARGET;
use alloc::format;
use core::marker::PhantomData;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use sp_runtime::RuntimeDebug;

pub struct MiningBridge<T>(PhantomData<T>);

#[evm_macro::generate_function_selector]
#[derive(RuntimeDebug, Eq, PartialEq, TryFromPrimitive, IntoPrimitive)]
#[repr(u32)]
pub enum Selector {
    // ── 注册 / 质押生命周期 ──
    SetSelfController = "setSelfController()",
    BondMachine = "bondMachine(string,bytes,bytes)",
    GenServerRoom = "genServerRoom()",
    AddMachineInfo = "addMachineInfo(string,bytes)",
    FulfillMachine = "fulfillMachine(string)",
    ClaimRewards = "claimRewards()",
    // ── controller 自救 / 运维（G2：映射账户无私钥，必须全暴露）──
    ControllerReportOffline = "controllerReportOffline(string)",
    ControllerReportOnline = "controllerReportOnline(string)",
    RestakeOnlineMachine = "restakeOnlineMachine(string)",
    MachineExit = "machineExit(string)",
    ApplySlashReview = "applySlashReview(uint64,bytes)", // 🔴 申诉不公罚没
    SetMachineExtraPrice = "setMachineExtraPrice(string,uint64)",
    UpdateMachineInfo = "updateMachineInfo(string,bytes)",
    OfflineMachineChangeHardwareInfo = "offlineMachineChangeHardwareInfo(string)",
    SetRentReceiver = "setRentReceiver(bytes)", // 空 bytes=None，32 字节=Some(AccountId)
}

impl<T> Precompile for MiningBridge<T>
where
    T: pallet_evm::Config + online_profile::Config,
{
    fn execute(handle: &mut impl PrecompileHandle) -> PrecompileResult {
        let input = handle.input();
        if input.len() < 4 {
            return Err(revert("invalid input"))
        }
        let selector = u32::from_be_bytes(input[..4].try_into().expect("checked len>=4. qed"));
        let selector: Selector = selector
            .try_into()
            .map_err(|e| revert(format!("invalid selector: {:?}", e)))?;
        let args = input.get(4..).unwrap_or_default();

        // [G1] caller 必须是 EOA：有代码=合约 → 拒绝（否则映射账户=中间合约影子账户，机器绑到不可达账户）
        let caller = handle.context().caller;
        if !pallet_evm::AccountCodes::<T>::get(caller).is_empty() {
            return Err(revert("MiningBridge: caller must be an EOA (contract calls forbidden)"))
        }
        // 映射账户 = stash == controller（本模型一钱包兼任）。origin 在各 dispatch 处内联构造
        //（RawOrigin::Signed(who.clone()).into() 的目标 OriginFor<T> 由被调 extrinsic 首参推断，故不用闭包避免返回类型无法推断）。
        let who: T::AccountId = <T as pallet_evm::Config>::AddressMapping::into_account_id(caller);

        match selector {
            // ── setSelfController：G5 幂等，已 set 过(controller_stash(who)=Some)则跳过 ──
            Selector::SetSelfController => {
                charge::<T>(handle, 4, 4)?;
                // TODO(DBC): 确认 controller_stash getter 的公开引用名；未 set 才调 set_controller
                if online_profile::Pallet::<T>::controller_stash(&who).is_none() {
                    dispatch(
                        online_profile::Pallet::<T>::set_controller(RawOrigin::Signed(who.clone()).into(), who.clone()),
                        "set_controller",
                    )?;
                }
                ok()
            },

            // ── bondMachine(machineId, msg, sig)：机器签名由 online_profile check_bonding_msg 校验；当场质押单卡额 ──
            Selector::BondMachine => {
                let p = decode(args, &[ParamType::String, ParamType::Bytes, ParamType::Bytes])?;
                let machine_id = as_string(&p, 0)?.into_bytes(); // MachineId = Vec<u8>(hex ASCII)
                let msg = as_bytes(&p, 1)?;
                let sig = as_bytes(&p, 2)?;
                charge::<T>(handle, 10, 10)?; // 质押 + 多处 StashMachines/LiveMachines/MachinesInfo 写
                dispatch(
                    online_profile::Pallet::<T>::bond_machine(RawOrigin::Signed(who.clone()).into(), machine_id, msg, sig),
                    "bond_machine",
                )?;
                ok()
            },

            Selector::GenServerRoom => {
                // [feng 建议] 机房 id 是随机 H256、客户端无法预测。直接调 pallet 内部实现拿到 id，
                //   作为 bytes32 EVM 返回值回传，客户端从 call 返回值直接读，省去解 ServerRoomGenerated 事件/绕存储回读。
                charge::<T>(handle, 3, 2)?; // G4：先记 gas 再改状态
                let room_id = online_profile::Pallet::<T>::do_gen_server_room(who.clone())
                    .map_err(|e| revert(format!("gen_server_room failed: {:?}", e)))?;
                ok_bytes32(room_id)
            },

            // ── addMachineInfo(machineId, StakerCustomizeInfo 的 SCALE 编码) ──
            Selector::AddMachineInfo => {
                let p = decode(args, &[ParamType::String, ParamType::Bytes])?;
                let machine_id = as_string(&p, 0)?.into_bytes();
                let room_info = decode_scale::<dbc_support::machine_type::StakerCustomizeInfo>(&as_bytes(&p, 1)?)?;
                charge::<T>(handle, 8, 6)?; // era 快照相关，给足
                dispatch(
                    online_profile::Pallet::<T>::add_machine_info(RawOrigin::Signed(who.clone()).into(), machine_id, room_info),
                    "add_machine_info",
                )?;
                ok()
            },

            Selector::FulfillMachine => {
                let machine_id = decode_single_string(args)?.into_bytes();
                charge::<T>(handle, 10, 8)?; // 补质押 + era 快照，重
                dispatch(
                    online_profile::Pallet::<T>::fulfill_machine(RawOrigin::Signed(who.clone()).into(), machine_id),
                    "fulfill_machine",
                )?;
                ok()
            },

            Selector::ClaimRewards => {
                charge::<T>(handle, 8, 6)?; // 领奖含部分再质押
                dispatch(online_profile::Pallet::<T>::claim_rewards(RawOrigin::Signed(who.clone()).into()), "claim_rewards")?;
                ok()
            },

            Selector::ControllerReportOffline => {
                let machine_id = decode_single_string(args)?.into_bytes();
                charge::<T>(handle, 5, 4)?;
                dispatch(
                    online_profile::Pallet::<T>::controller_report_offline(RawOrigin::Signed(who.clone()).into(), machine_id),
                    "controller_report_offline",
                )?;
                ok()
            },

            Selector::ControllerReportOnline => {
                let machine_id = decode_single_string(args)?.into_bytes();
                charge::<T>(handle, 6, 5)?;
                dispatch(
                    online_profile::Pallet::<T>::controller_report_online(RawOrigin::Signed(who.clone()).into(), machine_id),
                    "controller_report_online",
                )?;
                ok()
            },

            Selector::RestakeOnlineMachine => {
                let machine_id = decode_single_string(args)?.into_bytes();
                charge::<T>(handle, 6, 5)?;
                dispatch(
                    online_profile::Pallet::<T>::restake_online_machine(RawOrigin::Signed(who.clone()).into(), machine_id),
                    "restake_online_machine",
                )?;
                ok()
            },

            Selector::MachineExit => {
                let machine_id = decode_single_string(args)?.into_bytes();
                charge::<T>(handle, 8, 7)?;
                dispatch(
                    online_profile::Pallet::<T>::machine_exit(RawOrigin::Signed(who.clone()).into(), machine_id),
                    "machine_exit",
                )?;
                ok()
            },

            // ── 🔴 applySlashReview(slashId, reason)：controller 唯一的不公罚没链上防御 ──
            Selector::ApplySlashReview => {
                let p = decode(args, &[ParamType::Uint(64), ParamType::Bytes])?;
                let slash_id = as_u64(&p, 0)?; // TODO(DBC): SlashId 若非 u64 newtype 需转换
                let reason = as_bytes(&p, 1)?;
                charge::<T>(handle, 5, 3)?; // 申诉需从映射账户再押 slash_review_stake
                dispatch(
                    online_profile::Pallet::<T>::apply_slash_review(RawOrigin::Signed(who.clone()).into(), slash_id, reason),
                    "apply_slash_review",
                )?;
                ok()
            },

            Selector::SetMachineExtraPrice => {
                let p = decode(args, &[ParamType::String, ParamType::Uint(64)])?;
                let machine_id = as_string(&p, 0)?.into_bytes();
                let extra_price = as_u64(&p, 1)?;
                charge::<T>(handle, 3, 2)?;
                dispatch(
                    online_profile::Pallet::<T>::set_machine_extra_price(
                        RawOrigin::Signed(who.clone()).into(),
                        machine_id,
                        extra_price,
                    ),
                    "set_machine_extra_price",
                )?;
                ok()
            },

            Selector::UpdateMachineInfo => {
                let p = decode(args, &[ParamType::String, ParamType::Bytes])?;
                let machine_id = as_string(&p, 0)?.into_bytes();
                let room_info = decode_scale::<dbc_support::machine_type::StakerCustomizeInfo>(&as_bytes(&p, 1)?)?;
                charge::<T>(handle, 5, 4)?;
                dispatch(
                    online_profile::Pallet::<T>::update_machine_info(RawOrigin::Signed(who.clone()).into(), machine_id, room_info),
                    "update_machine_info",
                )?;
                ok()
            },

            Selector::OfflineMachineChangeHardwareInfo => {
                let machine_id = decode_single_string(args)?.into_bytes();
                charge::<T>(handle, 6, 5)?; // 触发重新验证
                dispatch(
                    online_profile::Pallet::<T>::offline_machine_change_hardware_info(
                        RawOrigin::Signed(who.clone()).into(),
                        machine_id,
                    ),
                    "offline_machine_change_hardware_info",
                )?;
                ok()
            },

            // ── setRentReceiver(bytes)：空=None，32 字节=Some(AccountId) ──
            Selector::SetRentReceiver => {
                let p = decode(args, &[ParamType::Bytes])?;
                let raw = as_bytes(&p, 0)?;
                let receiver: Option<T::AccountId> = if raw.is_empty() {
                    None
                } else {
                    Some(
                        T::AccountId::decode(&mut &raw[..])
                            .map_err(|_| revert("setRentReceiver: bad AccountId bytes"))?,
                    )
                };
                charge::<T>(handle, 2, 1)?;
                dispatch(
                    online_profile::Pallet::<T>::set_rent_receiver(RawOrigin::Signed(who.clone()).into(), receiver),
                    "set_rent_receiver",
                )?;
                ok()
            },
        }
    }
}

// ── ethabi 复用（与 rent_bridge 同）──
use ethabi::{decode as abi_decode, ParamType, Token};

fn decode(args: &[u8], types: &[ParamType]) -> Result<Vec<Token>, PrecompileFailure> {
    abi_decode(types, args).map_err(|e| revert(format!("decode param failed: {:?}", e)))
}
fn decode_single_string(args: &[u8]) -> Result<alloc::string::String, PrecompileFailure> {
    let p = decode(args, &[ParamType::String])?;
    as_string(&p, 0)
}
fn as_string(p: &[Token], i: usize) -> Result<alloc::string::String, PrecompileFailure> {
    p.get(i)
        .cloned()
        .and_then(|t| t.into_string())
        .ok_or_else(|| revert("decode string param failed"))
}
fn as_bytes(p: &[Token], i: usize) -> Result<Vec<u8>, PrecompileFailure> {
    p.get(i).cloned().and_then(|t| t.into_bytes()).ok_or_else(|| revert("decode bytes param failed"))
}
fn as_u64(p: &[Token], i: usize) -> Result<u64, PrecompileFailure> {
    p.get(i)
        .cloned()
        .and_then(|t| t.into_uint())
        .map(|u| u.low_u64())
        .ok_or_else(|| revert("decode uint param failed"))
}

// [G4] SCALE 解码失败必须干净 revert（不留残留）
fn decode_scale<D: Decode>(bytes: &[u8]) -> Result<D, PrecompileFailure> {
    D::decode(&mut &bytes[..]).map_err(|_| revert("SCALE decode failed (StakerCustomizeInfo?)"))
}

// [G3] 显式权重：先 record_cost（G4：改状态之前）；ExitError 经 ? 转 PrecompileFailure
fn charge<T: pallet_evm::Config>(
    handle: &mut impl PrecompileHandle,
    reads: u64,
    writes: u64,
) -> Result<(), ExitError> {
    let weight = Weight::default()
        .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(reads))
        .saturating_add(<T as frame_system::Config>::DbWeight::get().writes(writes));
    handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))
}

// DispatchResultWithPostInfo → PrecompileResult 的错误映射（把 online_profile 的 Error 透传给 EVM revert 文案）
fn dispatch(
    r: frame_support::dispatch::DispatchResultWithPostInfo,
    ctx: &str,
) -> Result<(), PrecompileFailure> {
    r.map(|_| ()).map_err(|e| {
        log::debug!(target: LOG_TARGET, "mining_bridge {} failed: {:?}", ctx, e.error);
        revert(format!("{} failed: {:?}", ctx, e.error))
    })
}

fn revert(msg: impl Into<Vec<u8>>) -> PrecompileFailure {
    PrecompileFailure::Revert { exit_status: ExitRevert::Reverted, output: msg.into() }
}

fn ok() -> PrecompileResult {
    Ok(PrecompileOutput { exit_status: ExitSucceed::Returned, output: Default::default() })
}

/// 返回一个 bytes32（H256 的 32 字节），供 genServerRoom() 把机房 id 直接回传给 EVM 客户端。
/// ABI 上 `bytes32` 就是 32 原始字节、无需额外 padding（H256 恰好 32 字节）。
fn ok_bytes32(h: H256) -> PrecompileResult {
    Ok(PrecompileOutput { exit_status: ExitSucceed::Returned, output: h.as_bytes().to_vec() })
}
