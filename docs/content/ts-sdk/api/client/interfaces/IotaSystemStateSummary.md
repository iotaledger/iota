# Interface: IotaSystemStateSummary

This is the JSON-RPC type for the IOTA system state object. It flattens all fields to make them
top-level fields such that it as minimum dependencies to the internal data structures of the IOTA
system state type.

## Properties

### activeValidators

> **activeValidators**: [`IotaValidatorSummary`](IotaValidatorSummary.md)[]

The list of active validators in the current epoch.

---

### atRiskValidators

> **atRiskValidators**: [`string`, `string`][]

Map storing the number of epochs for which each validator has been below the low stake threshold.

---

### epoch

> **epoch**: `string`

The current epoch ID, starting from 0.

---

### epochDurationMs

> **epochDurationMs**: `string`

The duration of an epoch, in milliseconds.

---

### epochStartTimestampMs

> **epochStartTimestampMs**: `string`

Unix timestamp of the current epoch start

---

### inactivePoolsId

> **inactivePoolsId**: `string`

ID of the object that maps from a staking pool ID to the inactive validator that has that pool as
its staking pool.

---

### inactivePoolsSize

> **inactivePoolsSize**: `string`

Number of inactive staking pools.

---

### iotaTotalSupply

> **iotaTotalSupply**: `string`

The current IOTA supply.

---

### iotaTreasuryCapId

> **iotaTreasuryCapId**: `string`

The `TreasuryCap<IOTA>` object ID.

---

### maxValidatorCount

> **maxValidatorCount**: `string`

Maximum number of active validators at any moment. We do not allow the number of validators in any
epoch to go above this.

---

### minValidatorCount

> **minValidatorCount**: `string`

Minimum number of active validators at any moment. We do not allow the number of validators in any
epoch to go under this.

---

### minValidatorJoiningStake

> **minValidatorJoiningStake**: `string`

Lower-bound on the amount of stake required to become a validator.

---

### pendingActiveValidatorsId

> **pendingActiveValidatorsId**: `string`

ID of the object that contains the list of new validators that will join at the end of the epoch.

---

### pendingActiveValidatorsSize

> **pendingActiveValidatorsSize**: `string`

Number of new validators that will join at the end of the epoch.

---

### pendingRemovals

> **pendingRemovals**: `string`[]

Removal requests from the validators. Each element is an index pointing to `active_validators`.

---

### protocolVersion

> **protocolVersion**: `string`

The current protocol version, starting from 1.

---

### referenceGasPrice

> **referenceGasPrice**: `string`

The reference gas price for the current epoch.

---

### safeMode

> **safeMode**: `boolean`

Whether the system is running in a downgraded safe mode due to a non-recoverable bug. This is set
whenever we failed to execute advance_epoch, and ended up executing advance_epoch_safe_mode. It can
be reset once we are able to successfully execute advance_epoch.

---

### safeModeComputationRewards

> **safeModeComputationRewards**: `string`

Amount of computation rewards accumulated (and not yet distributed) during safe mode.

---

### safeModeNonRefundableStorageFee

> **safeModeNonRefundableStorageFee**: `string`

Amount of non-refundable storage fee accumulated during safe mode.

---

### safeModeStorageCharges

> **safeModeStorageCharges**: `string`

Amount of storage charges accumulated (and not yet distributed) during safe mode.

---

### safeModeStorageRebates

> **safeModeStorageRebates**: `string`

Amount of storage rebates accumulated (and not yet burned) during safe mode.

---

### stakingPoolMappingsId

> **stakingPoolMappingsId**: `string`

ID of the object that maps from staking pool's ID to the iota address of a validator.

---

### stakingPoolMappingsSize

> **stakingPoolMappingsSize**: `string`

Number of staking pool mappings.

---

### storageFundNonRefundableBalance

> **storageFundNonRefundableBalance**: `string`

The non-refundable portion of the storage fund coming from non-refundable storage rebates and any
leftover staking rewards.

---

### storageFundTotalObjectStorageRebates

> **storageFundTotalObjectStorageRebates**: `string`

The storage rebates of all the objects on-chain stored in the storage fund.

---

### systemStateVersion

> **systemStateVersion**: `string`

The current version of the system state data structure type.

---

### totalStake

> **totalStake**: `string`

Total amount of stake from all active validators at the beginning of the epoch.

---

### validatorCandidatesId

> **validatorCandidatesId**: `string`

ID of the object that stores preactive validators, mapping their addresses to their `Validator`
structs.

---

### validatorCandidatesSize

> **validatorCandidatesSize**: `string`

Number of preactive validators.

---

### validatorLowStakeGracePeriod

> **validatorLowStakeGracePeriod**: `string`

A validator can have stake below `validator_low_stake_threshold` for this many epochs before being
kicked out.

---

### validatorLowStakeThreshold

> **validatorLowStakeThreshold**: `string`

Validators with stake amount below `validator_low_stake_threshold` are considered to have low stake
and will be escorted out of the validator set after being below this threshold for more than
`validator_low_stake_grace_period` number of epochs.

---

### validatorReportRecords

> **validatorReportRecords**: [`string`, `string`[]][]

A map storing the records of validator reporting each other.

---

### validatorVeryLowStakeThreshold

> **validatorVeryLowStakeThreshold**: `string`

Validators with stake below `validator_very_low_stake_threshold` will be removed immediately at
epoch change, no grace period.
