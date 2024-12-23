# Interface: ParallelTransactionExecutorOptions

## Extends

- `Omit`\<`ObjectCacheOptions`, `"address"`\>

## Properties

### cache?

> `optional` **cache**: [`AsyncCache`](../classes/AsyncCache.md)

#### Inherited from

`Omit.cache`

---

### client

> **client**: [`IotaClient`](../../client/classes/IotaClient.md)

---

### signer

> **signer**: [`Signer`](../../cryptography/classes/Signer.md)

---

### coinBatchSize?

> `optional` **coinBatchSize**: `number`

The number of coins to create in a batch when refilling the gas pool

---

### initialCoinBalance?

> `optional` **initialCoinBalance**: `bigint`

The initial balance of each coin created for the gas pool

---

### minimumCoinBalance?

> `optional` **minimumCoinBalance**: `bigint`

The minimum balance of a coin that can be reused for future transactions. If the gasCoin is below this value, it will be used when refilling the gasPool

---

### defaultGasBudget?

> `optional` **defaultGasBudget**: `bigint`

The gasBudget to use if the transaction has not defined it's own gasBudget, defaults to `minimumCoinBalance`

---

### epochBoundaryWindow?

> `optional` **epochBoundaryWindow**: `number`

Time to wait before/after the expected epoch boundary before re-fetching the gas pool (in milliseconds).
Building transactions will be paused for up to 2x this duration around each epoch boundary to ensure the
gas price is up-to-date for the next epoch.

---

### maxPoolSize?

> `optional` **maxPoolSize**: `number`

The maximum number of transactions that can be execute in parallel, this also determines the maximum number of gas coins that will be created

---

### sourceCoins?

> `optional` **sourceCoins**: `string`[]

An initial list of coins used to fund the gas pool, uses all owned IOTA coins by default
