# Interface: IotaValidatorSummary

This is the JSON-RPC type for the IOTA validator. It flattens all inner structures to top-level
fields so that they are decoupled from the internal definitions.

## Properties

### authorityPubkeyBytes

> **authorityPubkeyBytes**: `string`

---

### commissionRate

> **commissionRate**: `string`

---

### description

> **description**: `string`

---

### exchangeRatesId

> **exchangeRatesId**: `string`

ID of the exchange rate table object.

---

### exchangeRatesSize

> **exchangeRatesSize**: `string`

Number of exchange rates in the table.

---

### gasPrice

> **gasPrice**: `string`

---

### imageUrl

> **imageUrl**: `string`

---

### iotaAddress

> **iotaAddress**: `string`

---

### name

> **name**: `string`

---

### netAddress

> **netAddress**: `string`

---

### networkPubkeyBytes

> **networkPubkeyBytes**: `string`

---

### nextEpochAuthorityPubkeyBytes?

> `optional` **nextEpochAuthorityPubkeyBytes**: `null` \| `string`

---

### nextEpochCommissionRate

> **nextEpochCommissionRate**: `string`

---

### nextEpochGasPrice

> **nextEpochGasPrice**: `string`

---

### nextEpochNetAddress?

> `optional` **nextEpochNetAddress**: `null` \| `string`

---

### nextEpochNetworkPubkeyBytes?

> `optional` **nextEpochNetworkPubkeyBytes**: `null` \| `string`

---

### nextEpochP2pAddress?

> `optional` **nextEpochP2pAddress**: `null` \| `string`

---

### nextEpochPrimaryAddress?

> `optional` **nextEpochPrimaryAddress**: `null` \| `string`

---

### nextEpochProofOfPossession?

> `optional` **nextEpochProofOfPossession**: `null` \| `string`

---

### nextEpochProtocolPubkeyBytes?

> `optional` **nextEpochProtocolPubkeyBytes**: `null` \| `string`

---

### nextEpochStake

> **nextEpochStake**: `string`

---

### operationCapId

> **operationCapId**: `string`

---

### p2pAddress

> **p2pAddress**: `string`

---

### pendingPoolTokenWithdraw

> **pendingPoolTokenWithdraw**: `string`

Pending pool token withdrawn during the current epoch, emptied at epoch boundaries.

---

### pendingStake

> **pendingStake**: `string`

Pending stake amount for this epoch.

---

### pendingTotalIotaWithdraw

> **pendingTotalIotaWithdraw**: `string`

Pending stake withdrawn during the current epoch, emptied at epoch boundaries.

---

### poolTokenBalance

> **poolTokenBalance**: `string`

Total number of pool tokens issued by the pool.

---

### primaryAddress

> **primaryAddress**: `string`

---

### projectUrl

> **projectUrl**: `string`

---

### proofOfPossessionBytes

> **proofOfPossessionBytes**: `string`

---

### protocolPubkeyBytes

> **protocolPubkeyBytes**: `string`

---

### rewardsPool

> **rewardsPool**: `string`

The epoch stake rewards will be added here at the end of each epoch.

---

### stakingPoolActivationEpoch?

> `optional` **stakingPoolActivationEpoch**: `null` \| `string`

The epoch at which this pool became active.

---

### stakingPoolDeactivationEpoch?

> `optional` **stakingPoolDeactivationEpoch**: `null` \| `string`

The epoch at which this staking pool ceased to be active. `None` = {pre-active, active},

---

### stakingPoolId

> **stakingPoolId**: `string`

ID of the staking pool object.

---

### stakingPoolIotaBalance

> **stakingPoolIotaBalance**: `string`

The total number of IOTA tokens in this pool.

---

### votingPower

> **votingPower**: `string`
