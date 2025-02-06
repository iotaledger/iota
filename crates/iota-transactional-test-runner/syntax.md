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

Example:

```move
//# init --accounts acc1 acc2 --protocol-version 1 --simulator

//# view-object 0,0
```

`.exp` output:

```
processed 2 tasks

init:
acc1: object(0,0), acc2: object(0,1)

task 1 'view-object'. lines 3-3:
Owner: Account Address ( acc1 )
Version: 1
Contents: iota::coin::Coin<iota::iota::IOTA> {id: iota::object::UID {id: iota::object::ID {bytes: fake(0,0)}}, balance: iota::balance::Balance<iota::iota::IOTA> {value: 300000000000000u64}}
```

### `transfer-object`

The `TransferObject` subcommand is used to transfer ownership of an object from one account to another.

#### Syntax

```
//# transfer-object [OPTIONS] --recipient <RECIPIENT> <ID>
```

#### Options

```
<ID>: the ID of the object to be transferred.
--recipient <RECIPIENT_ADDRESS>: the address of the recipient.
--sender <SENDER> (optional): the sender's address (default is the default account).
--gas-budget <GAS> (optional): specifies the gas limit for the transaction.
--gas-price <PRICE> (optional): specifies the gas price.
```

Example:

```move
//# init --addresses test=0x0 --accounts acc1 acc2 --protocol-version 1

//# publish

module test::test_coin {
    public struct TestCoin has key, store {
        id: UID,
        amount: u64
    }

    public entry fun test_coin_mint(amount: u64, ctx: &mut TxContext) {
        let id = object::new(ctx);
        let test_coin = TestCoin { id, amount };
        transfer::public_transfer(test_coin, tx_context::sender(ctx));
    }
}

//# run test::test_coin::test_coin_mint --sender acc1 --args 2500

//# transfer-object 2,0 --sender acc1 --recipient acc2
```

`.exp` output:

```
processed 4 tasks

init:
acc1: object(0,0), acc2: object(0,1)

task 1 'publish'. lines 3-16:
created: object(1,0)
mutated: object(0,2)
gas summary: computation_cost: 1000000, storage_cost: 5586000,  storage_rebate: 0, non_refundable_storage_fee: 0

task 2 'run'. lines 18-18:
created: object(2,0)
mutated: object(0,0)
gas summary: computation_cost: 1000000, storage_cost: 2371200,  storage_rebate: 0, non_refundable_storage_fee: 0

task 3 'transfer-object'. lines 20-20:
mutated: object(0,0), object(2,0)
gas summary: computation_cost: 1000000, storage_cost: 2371200,  storage_rebate: 2371200, non_refundable_storage_fee: 0
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

Consensus commit prologue is available only in simulator mode.

Example:

```move
//# init --addresses test=0x0 --accounts acc1 acc2 --protocol-version 1 --simulator

//# consensus-commit-prologue --timestamp-ms 4500

//# view-object 6
```

`.exp` output:

```
processed 3 tasks

init:
acc1: object(0,0), acc2: object(0,1)

task 1 'consensus-commit-prologue'. lines 3-3:
mutated: 0x0000000000000000000000000000000000000000000000000000000000000006
gas summary: computation_cost: 0, storage_cost: 0,  storage_rebate: 0, non_refundable_storage_fee: 0

task 2 'view-object'. lines 5-5:
Owner: Shared
Version: 2
Contents: iota::clock::Clock {id: iota::object::UID {id: iota::object::ID {bytes: 0x0000000000000000000000000000000000000000000000000000000000000006}}, timestamp_ms: 4500u64}
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

Example:

```move
//# init --addresses test=0x0 --accounts acc1 acc2 --protocol-version 1

//# publish --sender acc1
module test::test_coin {
    public struct TestCoin has key, store {
        id: UID,
        amount: u64
    }

    public fun test_coin_mint(amount: u64, ctx: &mut TxContext) : TestCoin {
        let id = object::new(ctx);
        TestCoin { id, amount }
    }
}

//# programmable --sender acc1 --inputs 1000 @acc2
//> test::test_coin::test_coin_mint(Input(0));
//> TransferObjects([Result(0)], Input(1))
```

Here we're minting the `TestCoin` obj in fly and passing it to the `TransferObjects` ptb command via Result(0).

`.exp` output:

```
processed 3 tasks

init:
acc1: object(0,0), acc2: object(0,1)

task 1 'publish'. lines 3-14:
created: object(1,0)
mutated: object(0,0)
gas summary: computation_cost: 1000000, storage_cost: 5069200,  storage_rebate: 0, non_refundable_storage_fee: 0

task 2 'programmable'. lines 16-18:
created: object(2,0)
mutated: object(0,0)
gas summary: computation_cost: 1000000, storage_cost: 2371200,  storage_rebate: 988000, non_refundable_storage_fee: 0
```

The `programmable` subcommand is constructed using the same input, result and command components of a Programmable Transaction Block (PTB).

Inputs are the values you provide to the PTB, either as objects or pure values, while Results are the values produced by the commands within the PTB:
- `Input(u16)`:
- `Gas`:
- `Result(u16)`:
- `NestedResult(u16)`:

Commands encapsulates a specific operation with relevant arguments:

- `MoveCall(Box<ParsedMoveCall>)`: executes a Move function call with specified parameters. Use to call specific function in format `package::module::function` with appropriate args.
  Example: `//> test::test_coin::test_coin_mint(Input(0))`.
- `TransferObjects(Vec<Argument>, Argument)`: transfers one or more objects (`Vec<Argument>`) to a recipient (`Argument`).
  Example: `//> TransferObjects([Result(0)], Input(1))`.
- `SplitCoins(Argument, Vec<Argument>)`: splits a coin (`Argument`) into multiple smaller coins specified by `Vec<Argument>`.
  Example: `//> SplitCoins(Gas, [Input(0)])`
- `MergeCoins(Argument, Vec<Argument>)`: merges multiple coins (`Vec<Argument>`) into a single target coin (`Argument`).
  Example: `//> MergeCoins(Result(0), [Gas])`.
- `MakeMoveVec(Option<ParsedType>, Vec<Argument>)`: constructs a Move vector of a specific type (`Option<ParsedType>`) from a list of arguments (`Vec<Argument>`).
  Example: `//> MakeMoveVec<u64>([Input(0), Input(1)])`.
- `Publish(String, Vec<String>)`: publishes a new Move package, where the first String represents the package path, and `Vec<String>` contains dependencies.
- `Upgrade(String, Vec<String>, String, Argument)`: upgrades an existing Move package with a new version.
  First String: path to the upgraded package. `Vec<String>`: dependencies for the upgrade.
  Second String: digest of the previous package version.
  Argument: capability or authority required for the upgrade.

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

Example:

```move
//# init --addresses test=0x0 test2=0x0 --accounts acc1

//# publish --upgradeable --sender acc1
module test::test_coin {
    public struct TestCoin has key, store {
        id: UID,
        amount: u64
    }
}

//# upgrade --package test --upgrade-capability 1,0 --sender acc1
module test2::test_coin {
    public struct TestCoin has key, store {
        id: UID,
        amount: u64,
    }

    public fun mint() { }
}
```

`.exp` output:

```
processed 3 tasks

init:
acc1: object(0,0)

task 1 'publish'. lines 3-9:
created: object(1,0), object(1,1)
mutated: object(0,0)
gas summary: computation_cost: 1000000, storage_cost: 5958400,  storage_rebate: 0, non_refundable_storage_fee: 0

task 2 'upgrade'. lines 11-19:
created: object(2,0)
mutated: object(0,0), object(1,0)
gas summary: computation_cost: 1000000, storage_cost: 6171200,  storage_rebate: 2622000, non_refundable_storage_fee: 0
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

Example:

```move
//# init --addresses test=0x0 test2=0x0 --accounts acc1

//# stage-package
module test::test_coin {
    public struct TestCoin has key, store {
        id: UID,
        amount: u64
    }
}
```

`.exp` output:

```
processed 2 tasks

init:
acc1: object(0,0)
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

Example:

```move
//# init --addresses p=0x0 q=0x0 r=0x0 --accounts A

//# stage-package
module p::m {
    public fun foo(x: u64) {
        p::n::bar(x)
    }
}
module p::n {
    public fun bar(x: u64) {
        assert!(x == 0, 0);
    }
}


//# stage-package
module q::m {
    public fun x(): u64 { 0 }
}



//# programmable --sender A --inputs 10 @A
//> 0: SplitCoins(Gas, [Input(0)]);
//> 1: Publish(q, []);
//> 2: TransferObjects([Result(0)], Input(1));
//> 3: Publish(p, []);
//> TransferObjects([Result(1), Result(3)], Input(1))

//# view-object 3,3

//# view-object 3,4

//# set-address p object(3,4)

//# set-address q object(3,3)

//# programmable --sender A
//> 0: q::m::x();
//> p::m::foo(Result(0))

//# publish --dependencies p q
module r::all {
    public fun foo_x() {
        p::m::foo(q::m::x())
    }
}
```

`.exp` output:

```
processed 11 tasks

init:
A: object(0,0)

task 3 'programmable'. lines 23-28:
created: object(3,0), object(3,1), object(3,2), object(3,3), object(3,4)
mutated: object(0,0)
gas summary: computation_cost: 1000000, storage_cost: 8876800,  storage_rebate: 0, non_refundable_storage_fee: 0

task 4 'view-object'. lines 30-30:
3,3::m

task 5 'view-object'. lines 32-32:
3,4::{m, n}

task 8 'programmable'. lines 38-40:
mutated: object(0,0)
gas summary: computation_cost: 1000000, storage_cost: 988000,  storage_rebate: 988000, non_refundable_storage_fee: 0

task 9 'publish'. lines 42-47:
created: object(9,0)
mutated: object(0,1)
gas summary: computation_cost: 1000000, storage_cost: 5221200,  storage_rebate: 0, non_refundable_storage_fee: 0

task 10 'run'. lines 49-49:
mutated: object(0,1)
gas summary: computation_cost: 1000000, storage_cost: 988000,  storage_rebate: 988000, non_refundable_storage_fee: 0
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

Checkpoints creation is available only in simulator mode.

Example:

Creates a single checkpoint at the current state:

```move
//# init --accounts acc1 --simulator

//# create-checkpoint

//# view-checkpoint
```

`.exp` output:

```
processed 3 tasks

init:
acc1: object(0,0)

task 1 'create-checkpoint'. lines 3-3:
Checkpoint created: 1

task 2 'view-checkpoint'. lines 5-5:
CheckpointSummary { epoch: 0, seq: 1, content_digest: D3oWLCcqoa1D15gxzvMaDemNNY8YYVspAkYkcmtQKWRt,
            epoch_rolling_gas_cost_summary: GasCostSummary { computation_cost: 0, storage_cost: 0, storage_rebate: 0, non_refundable_storage_fee: 0 }}
```

Forces the creation of 5 checkpoints:

```
//# init --accounts acc1 --simulator

//# create-checkpoint 5

//# view-checkpoint
```

`.exp` output:

```
processed 3 tasks

init:
acc1: object(0,0)

task 1 'create-checkpoint'. lines 3-3:
Checkpoint created: 5

task 2 'view-checkpoint'. lines 5-5:
CheckpointSummary { epoch: 0, seq: 5, content_digest: D3oWLCcqoa1D15gxzvMaDemNNY8YYVspAkYkcmtQKWRt,
            epoch_rolling_gas_cost_summary: GasCostSummary { computation_cost: 0, storage_cost: 0, storage_rebate: 0, non_refundable_storage_fee: 0 }}
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

Full example:

```move
//# init --accounts acc1 --simulator

//# view-checkpoint

//# advance-epoch 10

//# view-checkpoint
```

`.exp` output:

```
processed 4 tasks

init:
acc1: object(0,0)

task 1 'view-checkpoint'. lines 3-3:
CheckpointSummary { epoch: 0, seq: 0, content_digest: 3XhwVx9s5eS29WHJN1AUcM4STEZREm2dpSP6nstENKJ2,
            epoch_rolling_gas_cost_summary: GasCostSummary { computation_cost: 0, storage_cost: 0, storage_rebate: 0, non_refundable_storage_fee: 0 }}

task 2 'advance-epoch'. lines 5-5:
Epoch advanced: 9

task 3 'view-checkpoint'. lines 7-7:
CheckpointSummary { epoch: 9, seq: 10, content_digest: BCyhwQbkWgfXXrYV4MKLDFA61sS79QrPQnxLXv5GnBsx,
            epoch_rolling_gas_cost_summary: GasCostSummary { computation_cost: 0, storage_cost: 0, storage_rebate: 0, non_refundable_storage_fee: 0 }}
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

Example:

```move
//# init --protocol-version 1 --simulator

//# create-checkpoint

// advance the clock by 1ms, next checkpoint timestmap should be 1970-01-01T00:00:00:001Z
//# advance-clock --duration-ns 1000000

//# create-checkpoint

// advance the clock by 1ms, next checkpoint timestmap should be 1970-01-01T00:00:00:002Z
//# advance-clock --duration-ns 1000000

//# create-checkpoint

// advance the clock by 1ms, next checkpoint timestmap should be 1970-01-01T00:00:00:003Z
//# advance-clock --duration-ns 1000000

//# create-checkpoint

// advance the clock by 10ms, next checkpoint timestmap should be 1970-01-01T00:00:00:013Z
//# advance-clock --duration-ns 10000000

//# create-checkpoint

// advance the clock by 2000ms, next checkpoint timestmap should be 1970-01-01T00:00:02:013Z
//# advance-clock --duration-ns 2000000000

//# create-checkpoint

// advance the clock by 990s / 16m30s, next checkpoint timestmap should be 1970-01-01T00:16:32.013Z
//# advance-clock --duration-ns 990000000000

//# create-checkpoint

// advance the clock by 9900s / 2h45m0s, next checkpoint timestmap should be 1970-01-01T03:01:32.013Z
//# advance-clock --duration-ns 9900000000000

//# advance-epoch

//# create-checkpoint

// advance the clock by 1888ms, next checkpoint timestmap should be 1970-01-01T03:01:33:901Z
//# advance-clock --duration-ns 1888000000

// advance the clock by 99ms, next checkpoint timestmap should be 1970-01-01T03:01:34:00Z
//# advance-clock --duration-ns 99000000

//# create-checkpoint

//# advance-epoch

//# run-graphql
{
  checkpoints(last: 10) {
    nodes {
      sequenceNumber
      timestamp
      epoch {
        epochId
      }
    }
  }
}

//# run-graphql
# Query for the system transaction that corresponds to a checkpoint (note that
# its timestamp is advanced, because the clock has advanced).
{
  transactionBlocks(last: 10) {
    nodes {
      kind {
        __typename
        ... on ConsensusCommitPrologueTransaction {
          epoch {
            epochId
          }
          commitTimestamp
          consensusCommitDigest
        }
      }
    }
  }
}
```

`.exp` output:

```
processed 23 tasks

task 1 'create-checkpoint'. lines 3-5:
Checkpoint created: 1

task 3 'create-checkpoint'. lines 8-10:
Checkpoint created: 2

task 5 'create-checkpoint'. lines 13-15:
Checkpoint created: 3

task 7 'create-checkpoint'. lines 18-20:
Checkpoint created: 4

task 9 'create-checkpoint'. lines 23-25:
Checkpoint created: 5

task 11 'create-checkpoint'. lines 28-30:
Checkpoint created: 6

task 13 'create-checkpoint'. lines 33-35:
Checkpoint created: 7

task 15 'advance-epoch'. lines 38-38:
Epoch advanced: 0

task 16 'create-checkpoint'. lines 40-42:
Checkpoint created: 9

task 19 'create-checkpoint'. lines 48-48:
Checkpoint created: 10

task 20 'advance-epoch'. lines 50-50:
Epoch advanced: 1

task 21 'run-graphql'. lines 52-63:
Response: {
  "data": {
    "checkpoints": {
      "nodes": [
        {
          "sequenceNumber": 2,
          "timestamp": "1970-01-01T00:00:00.001Z",
          "epoch": {
            "epochId": 0
          }
        },
        {
          "sequenceNumber": 3,
          "timestamp": "1970-01-01T00:00:00.002Z",
          "epoch": {
            "epochId": 0
          }
        },
        {
          "sequenceNumber": 4,
          "timestamp": "1970-01-01T00:00:00.003Z",
          "epoch": {
            "epochId": 0
          }
        },
        {
          "sequenceNumber": 5,
          "timestamp": "1970-01-01T00:00:00.013Z",
          "epoch": {
            "epochId": 0
          }
        },
        {
          "sequenceNumber": 6,
          "timestamp": "1970-01-01T00:00:02.013Z",
          "epoch": {
            "epochId": 0
          }
        },
        {
          "sequenceNumber": 7,
          "timestamp": "1970-01-01T00:16:32.013Z",
          "epoch": {
            "epochId": 0
          }
        },
        {
          "sequenceNumber": 8,
          "timestamp": "1970-01-01T03:01:32.013Z",
          "epoch": {
            "epochId": 0
          }
        },
        {
          "sequenceNumber": 9,
          "timestamp": "1970-01-01T03:01:32.013Z",
          "epoch": {
            "epochId": 1
          }
        },
        {
          "sequenceNumber": 10,
          "timestamp": "1970-01-01T03:01:34Z",
          "epoch": {
            "epochId": 1
          }
        },
        {
          "sequenceNumber": 11,
          "timestamp": "1970-01-01T03:01:34Z",
          "epoch": {
            "epochId": 1
          }
        }
      ]
    }
  }
}

task 22 'run-graphql'. lines 65-83:
Response: {
  "data": {
    "transactionBlocks": {
      "nodes": [
        {
          "kind": {
            "__typename": "ConsensusCommitPrologueTransaction",
            "epoch": {
              "epochId": 0
            },
            "commitTimestamp": "1970-01-01T00:00:00.002Z",
            "consensusCommitDigest": "11111111111111111111111111111111"
          }
        },
        {
          "kind": {
            "__typename": "ConsensusCommitPrologueTransaction",
            "epoch": {
              "epochId": 0
            },
            "commitTimestamp": "1970-01-01T00:00:00.003Z",
            "consensusCommitDigest": "11111111111111111111111111111111"
          }
        },
        {
          "kind": {
            "__typename": "ConsensusCommitPrologueTransaction",
            "epoch": {
              "epochId": 0
            },
            "commitTimestamp": "1970-01-01T00:00:00.013Z",
            "consensusCommitDigest": "11111111111111111111111111111111"
          }
        },
        {
          "kind": {
            "__typename": "ConsensusCommitPrologueTransaction",
            "epoch": {
              "epochId": 0
            },
            "commitTimestamp": "1970-01-01T00:00:02.013Z",
            "consensusCommitDigest": "11111111111111111111111111111111"
          }
        },
        {
          "kind": {
            "__typename": "ConsensusCommitPrologueTransaction",
            "epoch": {
              "epochId": 0
            },
            "commitTimestamp": "1970-01-01T00:16:32.013Z",
            "consensusCommitDigest": "11111111111111111111111111111111"
          }
        },
        {
          "kind": {
            "__typename": "ConsensusCommitPrologueTransaction",
            "epoch": {
              "epochId": 0
            },
            "commitTimestamp": "1970-01-01T03:01:32.013Z",
            "consensusCommitDigest": "11111111111111111111111111111111"
          }
        },
        {
          "kind": {
            "__typename": "EndOfEpochTransaction"
          }
        },
        {
          "kind": {
            "__typename": "ConsensusCommitPrologueTransaction",
            "epoch": {
              "epochId": 1
            },
            "commitTimestamp": "1970-01-01T03:01:33.901Z",
            "consensusCommitDigest": "11111111111111111111111111111111"
          }
        },
        {
          "kind": {
            "__typename": "ConsensusCommitPrologueTransaction",
            "epoch": {
              "epochId": 1
            },
            "commitTimestamp": "1970-01-01T03:01:34Z",
            "consensusCommitDigest": "11111111111111111111111111111111"
          }
        },
        {
          "kind": {
            "__typename": "EndOfEpochTransaction"
          }
        }
      ]
    }
  }
}
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

Example:

```move
//# init --protocol-version 1 --simulator

//# create-checkpoint

//# set-random-state --randomness-round 0 --random-bytes SGVsbG8gU3Vp --randomness-initial-version 2

//# create-checkpoint

//# run-graphql
{
    transactionBlocks(last: 1) {
        nodes {
            kind {
                __typename
                ... on RandomnessStateUpdateTransaction {
                    epoch { epochId }
                    randomnessRound
                    randomBytes
                    randomnessObjInitialSharedVersion
                }
            }
        }
    }
}
```

`.exp` output:

```
processed 5 tasks

task 1 'create-checkpoint'. lines 3-3:
Checkpoint created: 1

task 3 'create-checkpoint'. lines 7-7:
Checkpoint created: 2

task 4 'run-graphql'. lines 9-24:
Response: {
  "data": {
    "transactionBlocks": {
      "nodes": [
        {
          "kind": {
            "__typename": "RandomnessStateUpdateTransaction",
            "epoch": {
              "epochId": 0
            },
            "randomnessRound": 0,
            "randomBytes": "SGVsbG8gU3Vp",
            "randomnessObjInitialSharedVersion": 2
          }
        }
      ]
    }
  }
}
```

### `view-checkpoint`

The `ViewCheckpoint` subcommand retrieves and displays the latest checkpoint information from the blockchain. This is useful for debugging, monitoring, and ensuring data consistency across nodes.

#### Syntax

```
//# view-checkpoint
```

- Fetches the most recent checkpoint from the blockchain.
- Outputs details such as the checkpoint sequence number, epoch, digest, and gas info.

Example:

```move
//# init --accounts acc1 --simulator

//# view-checkpoint
```

`.exp` output:

```
processed 3 tasks

init:
acc1: object(0,0)

task 1 'view-checkpoint'. lines 3-3:
CheckpointSummary { epoch: 0, seq: 0, content_digest: 3XhwVx9s5eS29WHJN1AUcM4STEZREm2dpSP6nstENKJ2,
            epoch_rolling_gas_cost_summary: GasCostSummary { computation_cost: 0, storage_cost: 0, storage_rebate: 0, non_refundable_storage_fee: 0 }}
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

```move
//# init --accounts acc1 --simulator

//# create-checkpoint

//# force-object-snapshot-catchup --start-cp 0 --end-cp 1
```

`.exp` output:

```
processed 4 tasks

init:
acc1: object(0,0)

task 1 'create-checkpoint'. lines 3-3:
Checkpoint created: 1

task 2 'force-object-snapshot-catchup'. lines 5-5:
Objects snapshot updated to [0 to 1)
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

```move
//# init --addresses test=0x0 --accounts acc1 --protocol-version 1

//# publish --upgradeable --sender acc1
module test::test_coin {
    public struct TestCoin has key, store {
        id: UID,
        amount: u64
    }

    public entry fun test_coin_mint(amount: u64, ctx: &mut TxContext) {
        let id = object::new(ctx);
        let test_coin = TestCoin { id, amount };
        transfer::public_transfer(test_coin, tx_context::sender(ctx));
    }
}

//# bench test::test_coin::test_coin_mint --sender acc1 --args 10000000
```

This benchmarks test_coin_mint, executed by acc1.
Passes amount of coints to mint: 10000000.

`.exp` output:

```
processed 3 tasks

init:
acc1: object(0,0)

task 1 'publish'. lines 3-15:
created: object(1,0), object(1,1)
mutated: object(0,0)
gas summary: computation_cost: 1000000, storage_cost: 7220000,  storage_rebate: 0, non_refundable_storage_fee: 0
```

### `init`

### `print-bytecode`

> Translates the given Move IR module into bytecode, then prints a textual
> representation of that bytecode

### `publish`

### `run`
