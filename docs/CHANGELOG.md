# v2.2 -> v2.3:

## Runtime changed:

1. Added support for single card rental
2. New rent/renewal by minute (rent duration is integer multiple of 30 minutes)
3. Fixed a bug that may punish too much when the punishment is executed
4. After renting, the waiting time to confirm if the renting is successful is adjusted from 30 minutes to 15 minutes
5. RentMachine pallet Event Change
6. RPC changes to support single card rental, and serialization of some fields
7. Built-in sync node changes
8. Clean up documentation that duplicates with wiki
9. Add test cases
10. Code refactoring and optimization

### Data type changed：

```
# struct MachineInfo fields changed：
MachineInfo.last_machine_renter: Option<AccountId> -> MachineInfo.renters: Vec<AccountId>
MachineInfo.total_rented_duration: u64 -> MachineInfo.total_rented_duration: BlockNumber (单位从天变更到块)

# struct RentOrderDetail new fields：
machine_id: MachineId,
gpu_num: u32,
gpu_index: Vec<u32>,

# struct OPPendingSlashInfo new field:
renters: Vec<AccountId>
# field changed:
reward_to_reporter -> reporter

# MTReportInfoDetail new field：
rent_order_id：RentOrderId

# enum MachineFaultType one field changed:
enum MachineFaultType::RentedInaccessible(MachineId) -> MachineFaultType::RentedInaccessible(MachineId, RenterOrderId)
```

### Storage item in pallet changed：

```
onlineProfile
From `PendingExecMaxOfflineSlash = BlockNumber -> Vec<MachineId>` To `PendingOfflineSlash = (BlockNumber, MachineId) -> (Option<AccountId>, Vec<AccountId>)`

# `RentMachine` pallet storage changed：
From `UserRented = AccountId -> Vec<MachineId>` To `UserOrder = AccountId -> Vec<RentOrderId>`
From `RentOrder = MachineId -> RentOrderDetail` To `RentInfo = RentOrderId -> RentOrderDetail`
From `PendingConfirming = MachineId -> AccountId` To `ConfirmingOrder = BlockNumber -> Vec<RentOrderId>`
From `PendingRentEnding = BlockNumber -> Vec<MachineId>` To `RentEnding = BlockNumber -> Vec<RentOrderId>`

# `RentMachine` pallet new storage item:
MachineRentOrder = MachineId -> MachineGPUOrder
NextRentId = RentOrderId
```
