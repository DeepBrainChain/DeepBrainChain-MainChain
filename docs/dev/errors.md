# pallet errors

## rentMachine:
    * AccountAlreadyExist,            - account already exist
    * MachineNotRentable,             - machine not rentable
    * Overflow,                       - overflow
    * InsufficientValue,              - has insufficient value to pay or reserve
    * ExpiredConfirm,                 - confirm expired
    * NoOrderExist,                   - order not exist
    * StatusNotAllowed,               - when confirm a rent order, the machine is not online or rented
    * UnlockToPayFeeFailed,           - pay rent fee failed
    * UndefinedRentPot,               - the target which rent fee should be paid to is not defined
    * PayTxFeeFailed,                 - pay rent tx fee failed
    * GetMachinePriceFailed,          - get machine price failed
    * OnlyHalfHourAllowed,            - rent duration must be an integer multiple of 30 minutes
    * GPUNotEnough,                   - free GPU not enough
    * NotMachineRenter,               - not machine renter
    * Unknown,                        - machine or rent info not fount  
    * ReletTooShort,                  - Renewal duration too short, at least for 10 minutes or more


## onlineProfile:
    * BadSignature,                                 - signature verification failed
    * MachineIdExist,                               - machine id already exist
    * BalanceNotEnough,                             - balance not enough                
    * NotMachineController,                         - not machine controller
    * PayTxFeeFailed,                               - pay tx fee failed when bond a machine or generate a new seerver room
    * ClaimRewardFailed,                            - claim reward failed
    * ConvertMachineIdToWalletFailed,               - check bonding singature message failed
    * NoStashBond,                                  - stash account not bond
    * AlreadyController,                            - the account is already controller
    * NoStashAccount,                               - stash account not exist
    * BadMsgLen,                                    - bad message length when checking bonding singature message 
    * NotAllowedChangeMachineInfo,                  - not allowed to change machine info
    * MachineStashNotEqualControllerStash,          - machine stash not equal controller stash
    * CalcStakeAmountFailed,                        - calculate stake amount failed
    * SigMachineIdNotEqualBondedMachineId,          - signature machine id not equal bonded machine id when checking bonding singature message 
    * TelecomIsNull,                                - telecom is not found
    * MachineStatusNotAllowed,                      - machine status not allowed to change
    * ServerRoomNotFound,                           - server room not found
    * NotMachineStash,                              - not machine stash
    * TooFastToReStake,                             - too fast to re stake, should more than 1 year
    * NoStakeToReduce,                              - no stake to reduce
    * ReduceStakeFailed,                            - reduce stake failed
    * GetReonlineStakeFailed,                       - get re online stake failed
    * SlashIdNotExist,                              - slash id not exist
    * TimeNotAllowed,                               - machine online time less than 1 year can not be exit
    * ExpiredSlash,                                 - slash expired
    * Unknown,                                      - machine not found / pendding slash not found
    * ClaimThenFulfillFailed,                       - failed to supplement the insufficient reserve amount when claim reward 