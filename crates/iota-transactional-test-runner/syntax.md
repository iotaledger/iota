# Syntactic rules for mock network tasks in `iota-transactional-test-runner`

Transactional tests simulate network operations through the framework exposed in [iota-transactional-test-runner](https://github.com/iotaledger/iota/tree/develop/crates/iota-transactional-test-runner). The framework is actually built on top of the more generic [move-transactional-test-runner](https://github.com/iotaledger/iota/tree/develop/external-crates/move/crates/move-transactional-test-runner).

This currently used in the following tests:

```
$ cargo tree -i iota-transactional-test-runner
iota-transactional-test-runner v0.1.0 (crates/iota-transactional-test-runner)
[dev-dependencies]
├── iota-adapter-transactional-tests v0.1.0 (crates/iota-adapter-transactional-tests)
├── iota-graphql-e2e-tests v0.1.0 (crates/iota-graphql-e2e-tests)
└── iota-verifier-transactional-tests v0.1.0 (crates/iota-verifier-transactional-tests)
```

## Common rules

The framework introduces an ad-hoc syntax for defining network related operations/tasks as an extension to `move/mvir` files.

The syntax uses comments with the `//#` prefix to begin blocks of continuous non-empty lines that are eventually used to parse the underlying tasks and any additional `data`. Empty lines define the boundaries of each block. So the basic syntax for all tasks is the following:

```
<empty-line>
//# <task> [OPTIONS]
[<task-data>]
...
<empty-line>
```

For example:

```
                                                                        [empty-line]
//# run-graphql --show-usage --show-headers --show-service-version      [task]
{                                                                       [data]
  checkpoint {                                                          [data]
    sequenceNumber                                                      [data]
  }                                                                     [data]
}                                                                       [data]
                                                                        [empty-line]
```

The syntax rules for the `data` are specific to each task and will be discussed
in the respective sections.

## Supported tasks

### `view-object`

The `ViewObject` command retrieves and displays the details of a specific object stored on-chain. Objects can be Move resources, packages, or system objects.

#### Syntax

```
//# view-object <ID>
```

#### Options

```
<ID>: the ID of the object to be view.
```

### `transfer-object`

The `TransferObject` subcommand is used to transfer ownership of an object from one account to another.

#### Syntax

```
//# transfer-object [OPTIONS] --recipient <RECIPIENT> <ID>
```

#### Options

```
<ID>>: the ID of the object to be transferred.
--recipient <RECIPIENT_ADDRESS>: the address of the recipient.
--sender <SENDER> (optional): the sender's address (default is the default account).
--gas-budget <GAS> (optional): specifies the gas limit for the transaction.
--gas-price <PRICE> (optional): specifies the gas price.
```

### `consensus-commit-prologue`

The `ConsensusCommitPrologue` subcommand is used to commit a consensus event with a specific timestamp. It ensures that consensus-related operations maintain required order and timing.

#### Syntax

```
//# consensus-commit-prologue --timestamp-ms <<TIMESTAMP_MS>>
```

#### Options

```
-timestamp-ms <<TIMESTAMP_MS>>: specifies the timestamp (in milliseconds) at which the consensus event is committed. Commits a consensus event at the specific timestamp (which represents a specific moment in time in UTC).
```

### `programmable`

The `ProgrammableTransaction` subcommand allows executing a programmable transaction with custom inputs, commands, and optional simulation mode. This subcommand provides control over transaction execution.

#### Syntax

```
//# programmable [OPTIONS]
```

#### Options

```
--sender <SENDER> (optional): specifies the sender of the transaction. If omitted, the default account is used.
--gas-budget <GAS_BUDGET> (optional): defines the gas limit for executing the transaction. If omitted, a default gas budget is used.
--gas-price <GAS_PRICE> (optional): specifies the gas price for this transaction. If not set, the default gas price is used.
--dev-inspect (optional): runs the transaction in inspection mode without committing state changes.
--inputs <INPUTS>: a list of input arguments for the transaction. These inputs are passed as parameters to the commands executed in the programmable transaction.
```

### `upgrade`

The `UpgradePackage` subcommand is used to upgrade an existing Move package on-chain. This allows for adding new features, fixing bugs, or optimizing performance while maintaining compatibility with previous versions.

#### Syntax 

```
//# upgrade [OPTIONS] --package <PACKAGE> --upgrade-capability <UPGRADE_CAPABILITY> --sender <SENDER>
```

#### Options

```
--package <PACKAGE>: the name of the package to upgrade.
--upgrade-capability <UPGRADE_CAPABILITY>: the upgrade capability object that authorizes the upgrade.
--dependencies <DEPENDENCIES> (optional): a list of dependencies required for the upgraded package.
--sender <SENDER>: the account that submits the transaction.
--gas-budget <GAS_BUDGET>: the maximum amount of gas allowed for the upgrade transaction.
--syntax <SYNTAX> (optional): specifies the syntax type (source or ir). Defaults to source.
--policy <POLICY> (optional, default: compatible): the upgrade policy:
    compatible – Allows only compatible upgrades.
    additive – Allows adding new functionality but not modifying existing.
    dep_only – Allows only dependency updates.
--gas-price <GAS_PRICE> (optional): specifies the gas price for the transaction.
```

### `stage-package`

The `StagePackage` subcommand is used to prepare a package for future upgrades by staging it. Staging allows validation of the package's bytecode, dependencies, and structure before committing the upgrade. The package remains unpublished until explicitly upgraded.

#### Syntax 

```
//# stage-package [OPTIONS]
```

#### Options

```
--syntax <SYNTAX>: specifies the syntax type (source or ir).
--dependencies <DEPENDENCIES> (optional): a list of package dependencies required for the staged package.
```

### `set-address`

The `SetAddress` subcommand assigns a named address to an existing object, enabling it to be referenced by a human-readable identifier in subsequent commands. This is useful for improving readability and maintainability when working with objects in Move transactions.

#### Syntax 

```
//# set-address <NAME> <INPUT>
```

#### Options

```
<NAME>: the human-readable identifier for the address.
<INPUT>: the value to assign to the named address. This can be:
  A Move object (e.g., object(0x123))
  A digest of a staged package (e.g., digest(MyPackage))
  A receiving object (e.g., receiving(0x456))
  A shared immutable object (e.g., immshared(0x789))
```

### `create-checkpoint`

The `CreateCheckpoint` subcommand forces the creation of one or more checkpoints in the system. A checkpoint represents a snapshot of the system state at a specific point in time. It is useful for maintaining consistency, enabling recovery, and improving performance in blockchain-based environments.

#### Syntax 

```
//# create-checkpoint [COUNT]
```

#### Options

```
[COUNT]: specifies how many checkpoints to create. If omitted, a single checkpoint is created.
```

Example:

Creates a single checkpoint at the current state:
```
//# create-checkpoint
```

Forces the creation of 5 checkpoints:
```
//# create-checkpoint 5
```

### `advance-epoch`

The `AdvanceEpoch` subcommand manually advances the epoch in the system. Epochs represent discrete time periods in a network, and transitioning to a new epoch can involve validator set changes, protocol upgrades, and other governance actions.

#### Syntax 

```
//# advance-epoch [OPTIONS] [COUNT]
```

#### Options

```
[COUNT]: specifies the number of epochs to advance. If omitted, the command advances by one epoch.
--create-random-state: if set, generates a new random state when advancing the epoch.
```

Examples:

Advances the epoch by one step:
```
//# advance-epoch
```

Advances the epoch by 3 steps:
```
//# advance-epoch 3
```

Advances to the next epoch and generates a new random state:
```
//# advance-epoch --create-random-state
```

### `advance-clock`

The `AdvanceClock` subcommand manually advances the system clock by a specified duration. This is useful for testing time-dependent features like transaction expiration, staking rewards, and scheduled events.

#### Syntax 

```
//# advance-clock --duration-ns <DURATION_NS>
```

#### Options

```
--duration-ns <DURATION_NS>: specifies the duration (in nanoseconds) by which the clock should be advanced.
```

### `set-random-state`

The `SetRandomState` subcommand sets the blockchain's random state for testing and development purposes. It allows specifying a randomness round, input bytes for randomness, and an initial version number for tracking.

#### Syntax 

```
//# set-random-state --randomness-round <RANDOMNESS_ROUND> --random-bytes <RANDOM_BYTES> --randomness-initial-version <RANDOMNESS_INITIAL_VERSION>
```

#### Options

```
--randomness-round <RANDOMNESS_ROUND>: specifies the round number for which the randomness is being set.
--random-bytes <RANDOMNESS_BYTES>: the base64-encoded string representing the new randomness state.
--randomness-initial-version <RANDOMNESS_INITIAL_VERSION>: the version number at which this randomness state is initially set.
```

### `view-checkpoint`

The `ViewCheckpoint` subcommand retrieves and displays the latest checkpoint information from the blockchain. This is useful for debugging, monitoring, and ensuring data consistency across nodes.

#### Syntax 

```
//# view-checkpoint
```

 - Fetches the most recent checkpoint from the blockchain.
 - Outputs details such as the checkpoint sequence number, epoch, digest, and gas info.

Output:

```
CheckpointSummary { 
  epoch: 0, 
  seq: 0, 
  content_digest: Dg5SusgbrzKLK4MjPVQRob246bLroREvyUqp1bMEX7a9,
  epoch_rolling_gas_cost_summary: GasCostSummary { 
    computation_cost: 0, 
    storage_cost: 0, 
    storage_rebate: 0,
    non_refundable_storage_fee: 0 
  }
}
```

### `run-graphql`

Allows to execute GraphQL queries with optional options and returns the output.

#### Syntax:

```
//# run-graphql [OPTIONS]
<GraphQL-query>
```

#### Options:

```
--show-usage: Displays usage information for the command.
--show-headers: Includes HTTP headers in the output.
--show-service-version: Displays the version of the service handling the GraphQL query.
--cursors <cursor-list>: Specifies a list of cursors to be used within the query.
```

#### Query Interpolation

The command supports **query interpolation**, allowing you to dynamically replace parts of the GraphQL query at runtime.
It supports the following placeholders:

1. **Object Placeholders**
   - **Syntax**: `@{obj_x_y}` or `@{obj_x_y_opt}`
   - Here, `(x, y)` corresponds to the task index and the creation index of the object within that task. The placeholder will be replaced with the object ID as a string (like `0xABCD...`).

2. **Named Address Placeholders**
   - **Syntax**: `@{NamedAddr}` or `@{NamedAddr_opt}`
   - Substitutes known accounts and addresses that have been created during the initialization step, e.g. `init --protocol-version 1 --addresses P0=0x0 --accounts A B --simulator`

3. **Cursors**
   - **Syntax**: `//# run-graphql --cursors string1 string2 ...`
     - Depending on the query, the raw strings passed to `--cursors` might be required in JSON, BCS or any other format that the query expects.
     - Each string passed is automatically Base64-encoded (as all cursor values are expected to be Base64-encoded) and can be accessed in the query as `@{cursor_0}`, `@{cursor_1}`, etc., in the order provided.
     - To generate cursor values from objects at runtime, the strings passed must correspond to the format `@{obj_x_y}` or `@{obj_x_y, checkpoint}` and are translated to Base64-encoded object cursors.

All of the above rules (object placeholders, named address placeholders, cursor strings) can be used in a single query.
Any placeholder or cursor that cannot be mapped to a known variable, object, or address will cause an error.

#### Examples

The following example query will replace the placeholder `@{cursor_0}` with the Base64-encoded [transaction block cursor](../../crates/iota-graphql-rpc/src/types/transaction_block.rs) `{"c":3,"t":1,"tc":1}` where `c` is the checkpoint sequence number, `t` is the transaction sequence number, and `tc` is the transaction checkpoint number.
Cursor values depend on the query and the underlying schema. The cursor value above is specific to the GraphQL `transactionBlocks` query.
`@{A}` and `@{P0}` will be replaced with the addresses `A` and `P0` respectively that were created during the initialization step.

```
//# run-graphql --cursors {"c":3,"t":1,"tc":1}
{
  transactionBlocks(first: 1, after: "@{cursor_0}", filter: {signAddress: "@{A}"}) {
    nodes {
      sender {
        fakeCoinBalance: balance(type: "@{P0}::fake::FAKE") {
          totalBalance
        }
        allBalances: balances {
          nodes {
            coinType {
              repr
            }
            coinObjectCount
            totalBalance
          }
        }
      }
    }
  }
}
```

An example of a query that generates an object cursor at runtime:

```
//# run-graphql --cursors @{obj_6_0}
{
  address(address: "@{A}") {
    objects(first: 2 after: "@{cursor_0}") {
      edges {
        node {
          contents {
            json
          }
        }
      }
    }
  }
}
```

### `force-object-snapshot-catchup`

The `ForceObjectSnapshotCatchup` subcommand forces the system to catch up on object snapshots between a specified range of checkpoints. This is useful for ensuring that object state updates are fully synchronized across nodes, particularly in scenarios where snapshots may be lagging behind.

#### Syntax

```
//# force-object-snapshot-catchup -start-cp <START_CP> --end-cp <END_CP>
```

#### Options

```
--start-cp <START_CP>: the starting checkpoint sequence number from which to begin catching up object snapshots.
--end-cp <END_CP>: the ending checkpoint sequence number up to which object snapshots should be caught up.
```

Example:

```
//# force-object-snapshot-catchup --start-cp 100 --end-cp 110
```

 - Forces object snapshots to catch up from checkpoint 100 to checkpoint 110.
 - Ensures that any missed object state updates between these checkpoints are processed.

### `bench`

The `Bench` subcommand is used to benchmark a specific transaction execution. This is particularly useful for measuring the performance of a Move function execution by running it under benchmarking conditions.

#### Syntax 

```
//# bench [OPTIONS] [NAME]
```

#### Options

```
[NAME]: the name of the function to benchmark. If omitted, the transaction must be explicitly defined through other options. Expects 3 distinct parts - address, module, and struct.
--sender <SENDER>: the account that initiates the transaction. If omitted, the default sender account will be used.
--gas-price <GAS_PRICE>: specifies the gas price for the transaction execution.
--summarize: if set, produces a summarized output of the benchmark results instead of detailed logs.
--signers <SIGNERS>: a list of signers for the transaction, used when executing a function that requires multiple signers.
--args <ARGS>: a list of input arguments passed to the function being benchmarked. Arguments must match the expected input format.
--type-args <TYPE_ARGS>: specifies the type parameters used in the function execution.
--gas-budget <GAS_BUDGET>: sets the maximum amount of gas units allocated for the transaction execution.
--syntax <SYNTAX>: dfines the Move syntax type for transaction execution, either source (default) or IR.
```

Example:

```
//# bench 0x0::module::my_function --sender acc1 --gas-price 100 --gas-budget 5000000 --type-args 0x2::iota::IOTA
```
This benchmarks my_function, executed by acc1.
Uses a gas price of 100 and a gas budget of 5,000,000.
Passes one type argument: 0x2::iota::IOTA.

### `init`

### `print-bytecode`

> Translates the given Move IR module into bytecode, then prints a textual
> representation of that bytecode

### `publish`

### `run`
