v2.2 -> v2.3:

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
