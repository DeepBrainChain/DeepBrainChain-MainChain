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
    AbortBonding = "abortBonding(string)", // [审计修] 上线前中止绑定退质押（无私钥账户的 pre-online 自救口）
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

        // [审计修 M] 拒绝携带 value 的调用：本 precompile 无 payable 语义，转来的 DBC 会被永久困在
        //   2053 这个无私钥地址（EOA 手滑或 payable ABI 封装）。任何非零 value 直接 revert。
        if !handle.context().apparent_value.is_zero() {
            return Err(revert("MiningBridge: non-payable, do not send value"))
        }

        // [G1] caller 必须是 EOA：有代码=合约 → 拒绝（否则映射账户=中间合约影子账户，机器绑到不可达账户）
        let caller = handle.context().caller;
        if !pallet_evm::AccountCodes::<T>::get(caller).is_empty() {
            return Err(revert("MiningBridge: caller must be an EOA (contract calls forbidden)"))
        }
        // 映射账户 = stash == controller（本模型一钱包兼任）。origin 在各 dispatch 处内联构造
        //（RawOrigin::Signed(who.clone()).into() 的目标 OriginFor<T> 由被调 extrinsic 首参推断，故不用闭包避免返回类型无法推断）。
        let who: T::AccountId = <T as pallet_evm::Config>::AddressMapping::into_account_id(caller);

        match selector {
            // ── setSelfController：自控模型 who 既是 stash 又是 controller ──
            // [审计修 H1] 抢注防护。原来只判 controller_stash(who).is_none() 会被抢注绕过：攻击者预算受害者的
            //   映射 SS58 并 set_controller(origin=攻击者stash, controller=who) → ControllerStash[who]=攻击者stash。
            //   受害者 setSelfController 见 Some → 跳过、假成功(ok)，之后 bondMachine 读 controller_stash(who)=攻击者
            //   → 机器绑到攻击者 stash（抢奖励）。改为：仅当【已正确自绑】(StashController[who]==who && ControllerStash
            //   [who]==who) 才幂等成功；若被别的 stash 抢占(任一为 Some 但非 who) → revert 报错（不假成功、不盲绑），
            //   客户端换一个全新随机 0x 钱包重试即可（映射 SS58 = 客户端本地随机生成、攻击者无法预测未注册的地址）；
            //   只有干净状态(两者皆 None)才真正 set_controller 自绑。
            Selector::SetSelfController => {
                charge::<T>(handle, 4, 4)?;
                let sc = online_profile::Pallet::<T>::stash_controller(&who);
                let cs = online_profile::Pallet::<T>::controller_stash(&who);
                if sc.as_ref() == Some(&who) && cs.as_ref() == Some(&who) {
                    return ok() // 已正确自绑，幂等成功
                }
                if sc.is_some() || cs.is_some() {
                    // who 的 controller 映射被抢占/半绑（前置抢注）→ 不假成功、不盲绑
                    return Err(revert(
                        "MiningBridge: controller mapping for this address is already taken (front-run); use a fresh 0x wallet",
                    ))
                }
                dispatch(|| online_profile::Pallet::<T>::set_controller(RawOrigin::Signed(who.clone()).into(), who.clone()),
                    "set_controller",
                )?;
                ok()
            },

            // ── bondMachine(machineId, msg, sig)：机器签名由 online_profile check_bonding_msg 校验；当场质押单卡额 ──
            Selector::BondMachine => {
                let p = decode(args, &[ParamType::String, ParamType::Bytes, ParamType::Bytes])?;
                let machine_id = as_string(&p, 0)?.into_bytes(); // MachineId = Vec<u8>(hex ASCII)
                let msg = as_bytes(&p, 1)?;
                let sig = as_bytes(&p, 2)?;
                charge::<T>(handle, 10, 10)?; // 质押 + 多处 StashMachines/LiveMachines/MachinesInfo 写
                dispatch(|| online_profile::Pallet::<T>::bond_machine(RawOrigin::Signed(who.clone()).into(), machine_id, msg, sig),
                    "bond_machine",
                )?;
                ok()
            },

            Selector::GenServerRoom => {
                // [feng 建议 + 审计修 M/round2 G4] 机房 id 是随机 H256、客户端无法预测；EOA 直调 receipt 只含
                //   log、不含函数返回值，故生成后 emit event ServerRoomGenerated(address indexed miner, bytes32 roomId)
                //   把 id 带进 receipt（客户端按 topic0 过滤、从 data 读 32 字节；bytes32 返回值保留供合约/eth_call）。
                //   ★ G4 原子性：storage gas + LOG gas(375+375*2+8*32=1381) 全部在【改状态之前】record_cost，
                //   否则改完状态再 OOG 会留下扣费 + 孤儿房间（其 id 只由未发出的 log 携带、不可恢复）。
                charge::<T>(handle, 3, 2)?;
                handle
                    .record_cost(1381)
                    .map_err(|e| PrecompileFailure::Error { exit_status: e })?;
                let room_id = online_profile::Pallet::<T>::do_gen_server_room(who.clone())
                    .map_err(|e| revert(format!("gen_server_room failed: {:?}", e)))?;
                let mut miner_topic = [0u8; 32];
                miner_topic[12..].copy_from_slice(caller.as_bytes()); // H160 左补 12 零字节 = indexed address
                let event_addr = handle.code_address();
                handle
                    .log(
                        event_addr,
                        alloc::vec![
                            H256(*evm_macro::keccak256!("ServerRoomGenerated(address,bytes32)")),
                            H256(miner_topic),
                        ],
                        room_id.as_bytes().to_vec(),
                    )
                    .map_err(|e| PrecompileFailure::Error { exit_status: e })?;
                ok_bytes32(room_id)
            },

            // ── addMachineInfo(machineId, StakerCustomizeInfo 的 SCALE 编码) ──
            Selector::AddMachineInfo => {
                let p = decode(args, &[ParamType::String, ParamType::Bytes])?;
                let machine_id = as_string(&p, 0)?.into_bytes();
                let room_info = decode_scale::<dbc_support::machine_type::StakerCustomizeInfo>(&as_bytes(&p, 1)?)?;
                charge_era::<T>(handle, 8, 6)?; // era 快照相关，给足
                dispatch(|| online_profile::Pallet::<T>::add_machine_info(RawOrigin::Signed(who.clone()).into(), machine_id, room_info),
                    "add_machine_info",
                )?;
                ok()
            },

            Selector::FulfillMachine => {
                let machine_id = decode_single_string(args)?.into_bytes();
                // [DBC 权重修] fulfill_machine → fulfill_machine_stake 遍历 stash 的 online_machine（O(N) 读写，
                //   lib.rs:2306）。固定权重会在机器多时被低估 → 按 online 机器数线性计费，防低价放大 DoS。
                let n = online_profile::Pallet::<T>::stash_machines(&who).online_machine.len() as u64;
                charge_era::<T>(handle, 10u64.saturating_add(n.saturating_mul(2)), 8u64.saturating_add(n.saturating_mul(2)))?;
                dispatch(|| online_profile::Pallet::<T>::fulfill_machine(RawOrigin::Signed(who.clone()).into(), machine_id),
                    "fulfill_machine",
                )?;
                ok()
            },

            Selector::ClaimRewards => {
                // [DBC 权重修] claim_rewards 尾部同样调 fulfill_machine_stake（O(N)，lib.rs:881）→ 线性计费。
                let n = online_profile::Pallet::<T>::stash_machines(&who).online_machine.len() as u64;
                charge::<T>(handle, 8u64.saturating_add(n.saturating_mul(2)), 6u64.saturating_add(n.saturating_mul(2)))?;
                dispatch(|| online_profile::Pallet::<T>::claim_rewards(RawOrigin::Signed(who.clone()).into()), "claim_rewards")?;
                ok()
            },

            Selector::ControllerReportOffline => {
                let machine_id = decode_single_string(args)?.into_bytes();
                charge_era::<T>(handle, 5, 4)?;
                dispatch(|| online_profile::Pallet::<T>::controller_report_offline(RawOrigin::Signed(who.clone()).into(), machine_id),
                    "controller_report_offline",
                )?;
                ok()
            },

            Selector::ControllerReportOnline => {
                let machine_id = decode_single_string(args)?.into_bytes();
                charge_era::<T>(handle, 6, 5)?;
                dispatch(|| online_profile::Pallet::<T>::controller_report_online(RawOrigin::Signed(who.clone()).into(), machine_id),
                    "controller_report_online",
                )?;
                ok()
            },

            Selector::RestakeOnlineMachine => {
                let machine_id = decode_single_string(args)?.into_bytes();
                charge_era::<T>(handle, 6, 5)?;
                dispatch(|| online_profile::Pallet::<T>::restake_online_machine(RawOrigin::Signed(who.clone()).into(), machine_id),
                    "restake_online_machine",
                )?;
                ok()
            },

            Selector::MachineExit => {
                let machine_id = decode_single_string(args)?.into_bytes();
                charge_era::<T>(handle, 8, 7)?;
                dispatch(|| online_profile::Pallet::<T>::machine_exit(RawOrigin::Signed(who.clone()).into(), machine_id),
                    "machine_exit",
                )?;
                ok()
            },

            // [审计修] abortBonding：上线前(AddingCustomizeInfo)中止绑定并退质押。纯清理、无 era 快照 → 用普通 charge。
            Selector::AbortBonding => {
                let machine_id = decode_single_string(args)?.into_bytes();
                charge::<T>(handle, 5, 6)?;
                dispatch(|| online_profile::Pallet::<T>::abort_bonding(RawOrigin::Signed(who.clone()).into(), machine_id),
                    "abort_bonding",
                )?;
                ok()
            },

            // ── 🔴 applySlashReview(slashId, reason)：controller 唯一的不公罚没链上防御 ──
            Selector::ApplySlashReview => {
                let p = decode(args, &[ParamType::Uint(64), ParamType::Bytes])?;
                let slash_id = as_u64(&p, 0)?; // TODO(DBC): SlashId 若非 u64 newtype 需转换
                let reason = as_bytes(&p, 1)?;
                // [审计修 M/L] reason 会原样存进 PendingSlashReview（无界 Vec<u8>）→ 封顶 1KB 防状态膨胀，
                //   并按长度线性计 proof_size（每字节存储成本）。
                if reason.len() > 1024 {
                    return Err(revert("MiningBridge: applySlashReview reason too long (max 1024 bytes)"))
                }
                charge::<T>(handle, 5u64.saturating_add((reason.len() as u64) / 64), 3)?; // 申诉需从映射账户再押 slash_review_stake
                dispatch(|| online_profile::Pallet::<T>::apply_slash_review(RawOrigin::Signed(who.clone()).into(), slash_id, reason),
                    "apply_slash_review",
                )?;
                ok()
            },

            Selector::SetMachineExtraPrice => {
                let p = decode(args, &[ParamType::String, ParamType::Uint(64)])?;
                let machine_id = as_string(&p, 0)?.into_bytes();
                let extra_price = as_u64(&p, 1)?;
                charge::<T>(handle, 3, 2)?;
                dispatch(|| online_profile::Pallet::<T>::set_machine_extra_price(
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
                dispatch(|| online_profile::Pallet::<T>::update_machine_info(RawOrigin::Signed(who.clone()).into(), machine_id, room_info),
                    "update_machine_info",
                )?;
                ok()
            },

            Selector::OfflineMachineChangeHardwareInfo => {
                let machine_id = decode_single_string(args)?.into_bytes();
                charge_era::<T>(handle, 6, 5)?; // 触发重新验证
                dispatch(|| online_profile::Pallet::<T>::offline_machine_change_hardware_info(
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
                dispatch(|| online_profile::Pallet::<T>::set_rent_receiver(RawOrigin::Signed(who.clone()).into(), receiver),
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

// [审计修 round2 · 权重] 触及 era 快照的 op 专用计费。update_snap_on_online_changed / _on_rent_changed 读改
//   ErasStashPoints / ErasMachinePoints，其 staker_statistic 是按【全网所有质押人】的 BTreeMap，单条 blob 的
//   编码大小 / PoV proof_size 是 O(全网质押人)、与调用者无关——固定 DbWeight read/write 只计存储【次数】、
//   不计该大 blob 的 proof_size，导致这些 op 被严重低估（恒定 gas 触发无界读改 = 共识邻近的低价 DoS）。
//   故在 DbWeight 之上叠加一笔保守的大 proof_size + ref_time（覆盖较大质押人集合、并留增长余量）。
const ERA_SNAP_REF_TIME: u64 = 150_000_000;
const ERA_SNAP_PROOF_SIZE: u64 = 1_500_000;
fn charge_era<T: pallet_evm::Config>(
    handle: &mut impl PrecompileHandle,
    reads: u64,
    writes: u64,
) -> Result<(), ExitError> {
    let weight = Weight::from_parts(ERA_SNAP_REF_TIME, ERA_SNAP_PROOF_SIZE)
        .saturating_add(<T as frame_system::Config>::DbWeight::get().reads(reads))
        .saturating_add(<T as frame_system::Config>::DbWeight::get().writes(writes));
    handle.record_cost(T::GasWeightMapping::weight_to_gas(weight))
}

// [审计修 H2] 在事务性存储层里调用被派发的 pallet fn，并把错误映射成 EVM revert。
//   Frontier 在 EVM revert 时【不】回滚 Substrate 存储；且这里直接 pallet-fn 调用不经 executive 的
//   per-extrinsic 事务层 → 被调 fn 内部「先改状态、后续 `?` 失败」会留下半完成写（如 bond_machine 先扣
//   pay_fixed_tx_fee、再 change_stake 失败 → 手续费丢 + 无机器）。with_storage_layer 使 Err 时回滚所有
//   部分写，恢复原生 extrinsic 的原子性。入参改为闭包（否则被调 fn 在进事务层前就已执行、来不及回滚）。
fn dispatch<F>(f: F, ctx: &str) -> Result<(), PrecompileFailure>
where
    F: FnOnce() -> frame_support::dispatch::DispatchResultWithPostInfo,
{
    frame_support::storage::with_storage_layer(|| -> Result<(), sp_runtime::DispatchError> {
        f().map(|_| ()).map_err(|e| e.error)
    })
    .map_err(|e| {
        log::debug!(target: LOG_TARGET, "mining_bridge {} failed: {:?}", ctx, e);
        revert(format!("{} failed: {:?}", ctx, e))
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
