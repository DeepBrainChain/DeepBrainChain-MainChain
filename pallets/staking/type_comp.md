> v0.9.37新版本

```
ActiveEra = StorageValue<_, ActiveEraInfo>;                                                                                         | ActiveEra get(fn active_era): Option<ActiveEraInfo>;
Bonded = StorageMap<_, Twox64Concat, T::AccountId, T::AccountId>;                                                                   | Bonded get(fn bonded): map hasher(twox_64_concat) T::AccountId => Option<T::AccountId>;
BondedEras = StorageValue<_, Vec<(EraIndex, SessionIndex)>>;                                                                        | BondedEras: Vec<(EraIndex, SessionIndex)>;
CanceledSlashPayout = StorageValue<_, BalanceOf<T>>;                                                                                | CanceledSlashPayout get(fn canceled_payout) config(): BalanceOf<T>;
ChillThreshold = StorageValue<_, Percent>;
CurrentPlannedSession = StorageValue<_, SessionIndex>;                                                                              | 
CurrentEra = StorageValue<_, EraIndex>;                                                                                             | CurrentEra get(fn current_era): Option<EraIndex>;
ErasStartSessionIndex = StorageMap<_, Twox64Concat, EraIndex, SessionIndex>;                                                        | ErasStartSessionIndex get(fn eras_start_session_index): map EraIndex => Option<SessionIndex>;
ErasStakers = StorageDoubleMap<EraIndex, T::AccountId, Exposure<T::AccountId, BalanceOf<T>>>;        | ErasStakers get(fn eras_stakers): double_map EraIndex, T::AccountId => Exposure<T::AccountId, BalanceOf<T>>; 
ErasStakersClipped = StorageDoubleMap<_,EraIndex,T::AccountId,Exposure<T::AccountId, BalanceOf<T>>>; | ErasStakersClipped get(fn eras_stakers_clipped): double_map EraIndex, T::AccountId => Exposure<T::AccountId, BalanceOf<T>>;
ErasValidatorPrefs = StorageDoubleMap<EraIndex,T::AccountId,ValidatorPrefs>;                                                        | ErasValidatorPrefs get(fn eras_validator_prefs): double_map EraIndex, T::AccountId => ValidatorPrefs;
ErasValidatorReward = StorageMap<EraIndex, BalanceOf<T>>;                                                                           | ErasValidatorReward get(fn eras_validator_reward): map EraIndex => Option<BalanceOf<T>>;
ErasRewardPoints = StorageMap<EraIndex, EraRewardPoints<T::AccountId>>;                                                             | ErasRewardPoints get(fn eras_reward_points): map EraIndex => EraRewardPoints<T::AccountId>;
ErasTotalStake = StorageMap<EraIndex, BalanceOf<T>>;                                                                                | ErasTotalStake get(fn eras_total_stake): map EraIndex => BalanceOf<T>;
ForceEra = StorageValue<_, Forcing>;                                                                                                | ForceEra get(fn force_era) config(): Forcing;
Invulnerables = StorageValue<_, Vec<T::AccountId>>;                                                                                 | Invulnerables get(fn invulnerables) config(): Vec<T::AccountId>;
Ledger = StorageMap<_, T::AccountId, StakingLedger<T>>;                                                                             | Ledger get(fn ledger): map T::AccountId => Option<StakingLedger<T::AccountId, BalanceOf<T>>>;
MinimumValidatorCount = StorageValue<_, u32>;                                                                                       | MinimumValidatorCount get(fn minimum_validator_count) config(): u32;
MinNominatorBond = StorageValue<_, BalanceOf<T>>;
MinValidatorBond = StorageValue<_, BalanceOf<T>>;
MinimumActiveStake = StorageValue<_, BalanceOf<T>>;
MinCommission = StorageValue<_, Perbill>;
MaxValidatorsCount = StorageValue<_, u32>;
MaxNominatorsCount = StorageValue<_, u32>;
Nominators = CountedStorageMap<T::AccountId, Nominations<T>>;                                                                       | Nominators get(fn nominators): map T::AccountId => Option<Nominations<T::AccountId>>;
NominatorSlashInEra = StorageDoubleMap<EraIndex,T::AccountId, BalanceOf<T>>;                                                        | NominatorSlashInEra: double_map EraIndex, T::AccountId => Option<BalanceOf<T>>;
OffendingValidators = StorageValue<_, Vec<(u32, bool)>>;
Payee = StorageMap<_, Twox64Concat, T::AccountId, RewardDestination<T::AccountId>>;                                                 | Payee get(fn payee): map hasher(twox_64_concat) T::AccountId => RewardDestination<T::AccountId>;
SlashRewardFraction = StorageValue<_, Perbill>;                                                                                     | SlashRewardFraction get(fn slash_reward_fraction) config(): Perbill;
SlashingSpans = StorageMap<_, T::AccountId, slashing::SlashingSpans>;                                                               | SlashingSpans get(fn slashing_spans): map T::AccountId => Option<slashing::SlashingSpans>;
SpanSlash = StorageMap<(T::AccountId, slashing::SpanIndex), slashing::SpanRecord<BalanceOf<T>>>;                                    | SpanSlash: map (T::AccountId, slashing::SpanIndex) => slashing::SpanRecord<BalanceOf<T>>;
UnappliedSlashes = StorageMap<_,Twox64Concat,EraIndex,Vec<UnappliedSlash<T::AccountId, BalanceOf<T>>>>;                             | UnappliedSlashes: map hasher(twox_64_concat) EraIndex => Vec<UnappliedSlash<T::AccountId, BalanceOf<T>>>;
ValidatorSlashInEra = StorageDoubleMap<EraIndex,T::AccountId,(Perbill, BalanceOf<T>)>;                                              | ValidatorSlashInEra: double_map EraIndex, T::AccountId => Option<(Perbill, BalanceOf<T>)>;
Validators = CountedStorageMap<T::AccountId, ValidatorPrefs>;                                                                       | Validators get(fn validators): map T::AccountId => ValidatorPrefs;
ValidatorCount = StorageValue<_, u32>;                                                                                              | ValidatorCount get(fn validator_count) config(): u32;
```

> v3.0.0
```
EraElectionStatus get(fn era_election_status): ElectionStatus<T::BlockNumber>;
EarliestUnappliedSlash: Option<EraIndex>;
HistoryDepth get(fn history_depth) config(): u32 = 84;
IsCurrentSessionFinal get(fn is_current_session_final): bool = false;
QueuedElected get(fn queued_elected): Option<ElectionResult<T::AccountId, BalanceOf<T>>>;
QueuedScore get(fn queued_score): Option<ElectionScore>;
SnapshotValidators get(fn snapshot_validators): Option<Vec<T::AccountId>>;
SnapshotNominators get(fn snapshot_nominators): Option<Vec<T::AccountId>>;
StorageVersion build(|_: &GenesisConfig<T>| Releases::V5_0_0): Releases;
```
