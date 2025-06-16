# Syntactic Rules for Mock Network Tasks in `iota-transactional-test-runner`

# Content

- [Common Rules](#common-rules)
  - [ObjectID Rules](#objectid-rules)
    - [Understanding Object Identifiers (object(x,y))](#understanding-object-identifiers-objectxy)
    - [How IDs Are Assigned](#how-ids-are-assigned)
    - [Versioned Object Identifiers (object(x,y)@version)](#versioned-object-identifiers-objectxyversion)
    - [Usage Example](#usage-example)
- [Supported Tasks](#supported-tasks)
  - [`init`](#init)
  - [`print-bytecode`](#print-bytecode)
  - [`publish`](#publish)
  - [`run`](#run)
  - [`view-object`](#view-object)
  - [`transfer-object`](#transfer-object)
  - [`consensus-commit-prologue`](#consensus-commit-prologue)
  - [`programmable`](#programmable)
  - [`upgrade`](#upgrade)
  - [`stage-package`](#stage-package)
  - [`set-address`](#set-address)
  - [`create-checkpoint`](#create-checkpoint)
  - [`advance-epoch`](#advance-epoch)
  - [`advance-clock`](#advance-clock)
  - [`set-random-state`](#set-random-state)
  - [`view-checkpoint`](#view-checkpoint)
  - [`run-graphql`](#run-graphql)
  - [`bench`](#bench)
- [How `run_test` Compares a Move File With the Corresponding `.exp` File](#how-run_test-compares-a-move-file-with-the-corresponding-exp-file)
  - [Execution Process in `handle_actual_output`](#execution-process-in-handle_actual_output)
  - [Verification Process in `handle_expected_output`](#verification-process-in-handle_expected_output)
  - [Structure of the `.move` File](#structure-of-the-move-file)
  - [Structure of a `.exp` File](#structure-of-a-exp-file)
  - [Extending `handle_subcommand` and Creating New Subcommands](#extending-handle_subcommand-and-creating-new-subcommands)

Transactional tests simulate network operations through the framework exposed in [iota-transactional-test-runner](https://github.com/iotaledger/iota/tree/develop/crates/iota-transactional-test-runner). The framework is actually built on top of the more generic [move-transactional-test-runner](https://github.com/iotaledger/iota/tree/develop/external-crates/move/crates/move-transactional-test-runner).

This is currently used in the following tests:

```
$ cargo tree -i iota-transactional-test-runner
iota-transactional-test-runner v0.1.0 (crates/iota-transactional-test-runner)
[dev-dependencies]
├── iota-adapter-transactional-tests v0.1.0 (crates/iota-adapter-transactional-tests)
├── iota-graphql-e2e-tests v0.1.0 (crates/iota-graphql-e2e-tests)
└── iota-verifier-transactional-tests v0.1.0 (crates/iota-verifier-transactional-tests)
```

## Common Rules

The framework introduces an ad hoc syntax for defining network-related operations/tasks as an extension to `move/mvir` files.

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

### ObjectID Rules

Object identifiers (ObjectID) follow specific conventions that allow referencing objects across different test commands. This section describes how object IDs work, including how they are used in subcommands and programmable transactions (PTBs).

#### Understanding Object Identifiers (object(x,y))

Object identifiers in test files typically take the form:

```
object(x,y)
```

- x: Represents the task number in which the object was created.
- y: Represents the index of the object within that task.

For instance, object(1,0) means:

The object was created in task 1.
It was the first object (0-based index) created within that task.
In `.move` test files, object references are often written as:

```move
//# view-object 1,0
```

Here, 1,0 refers to the object created in task 1, index 0.

However, the index order can change due to test execution differences, such as:

- Non-deterministic transaction execution: Some transactions might reorder operations internally, leading to different assignment orders.
- Dynamic object discovery: Unwrapped objects (e.g., from storage) may be assigned new fake IDs later in the test, shifting the enumeration order.

##### How IDs Are Assigned

```rust
fn enumerate_fake(&mut self, id: ObjectID) -> FakeID {
    if let Some(fake) = self.object_enumeration.get_by_left(&id) {
        return *fake;
    }
    let (task, i) = self.next_fake;
    let fake_id = FakeID::Enumerated(task, i);
    self.object_enumeration.insert(id, fake_id);

    self.next_fake = (task, i + 1);
    fake_id
}
```

Each object gets a fake ID `FakeID::Enumerated(task, i)`, where task is the test task `index` and `i` is the object counter.
Objects are assigned in order as they are discovered during execution.
The next object `ID` increments (i + 1).

Why the Order Can Change:

```rust
async fn execute_txn(&mut self, transaction: Transaction) -> anyhow::Result<TxnSummary> {
    let mut created_ids: Vec<_> = effects
        .created()
        .iter()
        .map(|((id, _, _), _)| *id)
        .collect();
    let mut unwrapped_ids: Vec<_> = effects
        .unwrapped()
        .iter()
        .map(|((id, _, _), _)| *id)
        .collect();

    // Assign fake IDs to newly discovered objects
    let mut might_need_fake_id: Vec<_> = created_ids.iter().chain(unwrapped_ids.iter()).copied().collect();
    might_need_fake_id.sort_by_key(|id| self.get_object_sorting_key(id));

    for id in might_need_fake_id {
        self.enumerate_fake(id);
    }
}
```

- Unwrapped objects (effects.unwrapped()) are discovered later in execution.
- Created objects (effects.created()) are assigned based on transaction execution order.
- Sorting by get_object_sorting_key ensures determinism but relies on object properties.

#### Versioned Object Identifiers (object(x,y)@version)

Object references in PTBs can include a version number.

Example:

```
//# programmable --sender A --inputs object(1,0)@2 @acc1
//> TransferObjects([Input(0)], Input(1))
```

Here:

- @2: Indicates version 2 of the object.

Why specify versions?

- Objects mutate over time, especially in transactions.
- If an object is referenced in different states, the version ensures the correct state is used.
- If omitted, the latest known version of the object is used.

#### Usage example

`.move` file example:

```move
//# init --addresses P0=0x0 --accounts A --protocol-version 1 --simulator

//# programmable --sender A --inputs 1000 @A
//> SplitCoins(Gas, [Input(0)]);
//> TransferObjects([Result(0)], Input(1))

//# view-object 1,0

//# create-checkpoint

//# programmable --sender A --inputs object(1,0)@2
//> MergeCoins(Gas, [Input(0)])
```

`.exp` file example:

```
processed 5 tasks

init:
A: object(0,0)

task 1 'programmable'. lines 3-5:
created: object(1,0)
mutated: object(0,0)
gas summary: computation_cost: 1000000, storage_cost: 1976000,  storage_rebate: 0, non_refundable_storage_fee: 0

task 2 'view-object'. lines 7-7:
Owner: Account Address ( A )
Version: 2
Contents: iota::coin::Coin<iota::iota::IOTA> {id: iota::object::UID {id: iota::object::ID {bytes: fake(1,0)}}, balance: iota::balance::Balance<iota::iota::IOTA> {value: 1000u64}}

task 3 'create-checkpoint'. lines 9-9:
Checkpoint created: 1

task 4 'programmable'. lines 11-12:
mutated: object(0,0)
deleted: object(1,0)
gas summary: computation_cost: 1000000, storage_cost: 988000,  storage_rebate: 1976000, non_refundable_storage_fee: 0
```

Explanation:

In task 1:

- Object object(0,0) is the gas coin created for account address A in Task 0 and this is split.
- Object object(1,0) is created out of the split.

In task 4:

- object(0,0) is mutated.
- object(1,0) is deleted after being merged into object(0,0).

#### Summary

1. object(x,y): References an object created in task x, at index y.
2. view-object x,y: Displays the current state of the object.
3. object(x,y)@version: Specifies a particular version of the object.
4. Objects in PTBs: Used for transfers (TransferObjects), merging (MergeCoins), and execution of transactions.

## Supported Tasks

### `init`

The `init` command initializes the Move test environment. This command is used to set up various parameters such as named addresses, protocol versions, gas limits, and execution settings.

This command is **optional**, but if used, it must be the first command in the test sequence.

You should use the command:

- Before running any transactions in a test environment.
- When testing different protocol versions or gas pricing models.
- When working with named accounts and pre-defined addresses.
- For debugging storage behavior with object snapshots.

#### Syntax

```
//# init [OPTIONS]
```

#### Example

```
//# init --accounts acc1 acc2 --addresses test=0x0 --protocol-version 1 --simulator
```

- Creates two accounts: acc1 and acc2.
- Uses protocol version 1.
- Map numerical address to the named representation in order to use named alias.
- Runs in simulator mode for controlled testing.

`.exp` output:

```
+processed 1 task
+
+init:
+acc1: object(0,0), acc2: object(0,1)
```

#### Options

```
--accounts <ACCOUNTS>: defines a set of named accounts that will be created for testing. Each account is assigned an IOTA address and an associated gas object.
--protocol-version <PROTOCOL_VERSION>: specifies the protocol version to use for execution If not set, the highest available version is used.
--max-gas <MAX_GAS>: sets the maximum gas allowed per transaction. Only valid in non-simulator mode.
--shared-object-deletion <SHARED_OBJECT_DELETION>: enables or disables the deletion of shared objects during execution.
--simulator: runs the test adapter in simulator mode, allowing manual control over checkpoint creation and epoch advancement.
--custom-validator-account: creates a custom validator account. This is only allowed in simulator mode.
--reference-gas-price <REFERENCE_GAS_PRICE>: Defines a reference gas price for transactions. Only valid in simulator mode.
--default-gas-price <DEFAULT_GAS_PRICE>: sSets the default gas price for transactions. If not specified, the default is `1_000`.
--objects-snapshot-min-checkpoint-lag <OBJECT_SNAPSHOT_MIN_CHECKPOINT_LAG>: defines the minimum checkpoint lag for object snapshots. This affects when state snapshots are taken during execution
--flavor <FLAVOR>: Specifies the Move compiler flavor (e.g., Iota).
The --flavor option in the init command specifies the Move language flavor that will be used in the environment. This option determines the syntax and semantics applied to Move programs and packages in the test adapter(Core or Iota).
--addresses <NAMED_ADDRESSES>: Maps custom named addresses to specific numerical addresses for the Move environment.
```

#### What is the simulator mode?

This type of execution allows control of the checkpoint and epoch creation process and manually advances the clock as needed.
The simulator mode can be used when you need to debug shared objects or complex Move modules without waiting for full consensus validation.
You want full control over checkpointing and epochs for testing state transitions.

### `print-bytecode`

A command that reads a compiled Move binary and prints its bytecode instructions in a readable format.

> Translates the given Move IR module into bytecode, then prints a textual
> representation of that bytecode

#### Syntax

```
//# print-bytecode
```

#### Example

```
//# print-bytecode
module 0x0::transfer {
    public struct TestCoin has key, store {
        id: UID,
        amount: u64
    }

    public entry fun transfer(to: address, amount: u64, ctx: &mut TxContext) {
        let balance = 100;
        assert!(balance >= amount, 1);
        let id = object::new(ctx);
        let test_coin = TestCoin { id, amount };
        transfer::public_transfer(test_coin, to);
    }
}
```

Output bytecode:

```
processed 1 task

task 0 'print-bytecode'. lines 1-15:
// Move bytecode v6
module 0.transfer {
use 0000000000000000000000000000000000000000000000000000000000000002::object;
use 0000000000000000000000000000000000000000000000000000000000000002::transfer as 1transfer;
use 0000000000000000000000000000000000000000000000000000000000000002::tx_context;


struct TestCoin has store, key {
       id: UID,
       amount: u64
}

entry public transfer(to#0#0: address, amount#0#0: u64, ctx#0#0: &mut TxContext) {
B0:
       0: LdU64(100)
       1: CopyLoc[1](amount#0#0: u64)
       2: Ge
       3: BrFalse(5)
B1:
       4: Branch(9)
B2:
       5: MoveLoc[2](ctx#0#0: &mut TxContext)
       6: Pop
       7: LdU64(1)
       8: Abort
B3:
       9: MoveLoc[2](ctx#0#0: &mut TxContext)
       10: Call object::new(&mut TxContext): UID
       11: MoveLoc[1](amount#0#0: u64)
       12: Pack[0](TestCoin)
       13: MoveLoc[0](to#0#0: address)
       14: Call 1transfer::public_transfer<TestCoin>(TestCoin, address)
       15: Ret
}
}
```

#### Options

```
--syntax <SYNTAX>: move syntax type (`source` or `ir`).
```

- The `Move IR` is a low-level intermediate representation that closely mirrors Move bytecode. The Move bytecode defines programs published to the blockchain. What mainly differentiates the Move IR from the bytecode is that names are used as opposed to indexes into pools/tables.
- The `Move source` language is a high-level language that compiles to Move bytecode. It is designed to be a familiar and ergonomic language for developers that provides minimal abstractions over the Move bytecode.

Example of `.mvir` code:

```mvir
module 0x0.m {
    import 0x2.clock;

    public entry yes_clock_ref(l0: &clock.Clock) {
        label l0:
        abort 0;
    }
}
```

### `publish`

The `publish` command allows users to publish Move packages to the IOTA network. This command compiles the specified Move package and deploys it to the network, optionally marking it as upgradable.

#### Syntax

```
//# publish [OPTIONS]
```

#### Example

```move
//# publish --sender acc1 --upgradeable --gas-price 1000
module test::transfer {
    public struct TestCoin has key, store {
        id: UID,
        amount: u64
    }

    public entry fun transfer(to: address, amount: u64, ctx: &mut TxContext) {
        let balance = 100;
        assert!(balance >= amount, 1);
        let id = object::new(ctx);
        let test_coin = TestCoin { id, amount };
        transfer::public_transfer(test_coin, to);
    }
}
```

- Publishes `transfer.move` on-chain.
- `acc1` is the sender.
- The module is marked as upgradeable.
- Gas price is set to 1000.

`.exp` output:

```
+task 1 'publish'. lines 3-17:
+created: object(1,0), object(1,1)
+mutated: object(0,0)
+gas summary: computation_cost: 1000000, storage_cost: 7083200,  storage_rebate: 0, non_refundable_storage_fee: 0
```

#### Options

```
--sender <SENDER>: specifies the account that will be used to publish the package. If not provided, the default account is used.
--upgradeable: if specified, the package will be published as upgradeable, meaning it can be upgraded later with the `upgrade` command.
--dependencies <DEPENDENCIES>: a list of package dependencies that this package relies on. These dependencies should already be published
--gas-price <GAS_PRICE>: specifies the gas price to use for the transaction. If not provided, the default gas price is used
--gas-budget <GAS_BUDGET>: gas limit for execution
--syntax <SYNTAX>: move syntax type (`source` or `ir`).
```

### `run`

The `run` command is used to execute a function from a Move module.

#### Syntax

```
//# run [OPTIONS] [NAME]
```

`[NAME]` specified - `<ADDRESS>::<MODULE_NAME>::<FUNCTION_NAME>`

#### Options

```
--sender <SENDER>: defines the account initiating the transaction.
--gas-price <GAS_PRICE>: specifies the gas price for the transaction.
--summarize: enables summarized output of execution results
--signers <SIGNERS>: specifies who signs the transaction.
--args <ARGS>: specific arguments to pass into the function.
--type-args <TYPE_ARGS>: type arguments for generic functions.
--gas-budget <GAS_BUDGET>: gas limit for execution.
--syntax <SYNTAX>: move syntax type (`source` or `ir`).
```

#### Example

```move
//# init --addresses test=0x0 --accounts acc1 acc2 --protocol-version 1

//# publish

module test::transfer {
    public struct TestCoin has key, store {
        id: UID,
        amount: u64
    }

    public entry fun transfer(to: address, amount: u64, ctx: &mut TxContext) {
        let balance = 100;
        assert!(balance >= amount, 1);
        let id = object::new(ctx);
        let test_coin = TestCoin { id, amount };
        transfer::public_transfer(test_coin, to);
    }
}

//# run test::transfer::transfer --sender acc1 --gas-price 500 --args @acc2 50

//# view-object 2,0
```

`test::transfer` should have been published already before you can `run` the command execution.

- Runs transfer function.
- `acc1` is the sender.
- `@acc2` is an identifier of recipient address.
- `50` is an amount of tokens to mint.
- The gas price is set to 500.

`.exp` output:

```
processed 4 tasks

init:
acc1: object(0,0), acc2: object(0,1)

task 1 'publish'. lines 3-18:
created: object(1,0)
mutated: object(0,2)
gas summary: computation_cost: 1000000, storage_cost: 5449200,  storage_rebate: 0, non_refundable_storage_fee: 0

task 2 'run'. lines 20-20:
created: object(2,0)
mutated: object(0,0)
gas summary: computation_cost: 500000, storage_cost: 2363600,  storage_rebate: 0, non_refundable_storage_fee: 0

task 3 'view-object'. lines 22-22:
Owner: Account Address ( acc2 )
Version: 2
Contents: test::transfer::TestCoin {id: iota::object::UID {id: iota::object::ID {bytes: fake(2,0)}}, amount: 50u64}
```

### `view-object`

The `view-object` subcommand (`ViewObject` in Rust) retrieves and displays the details of a specific object stored on-chain. Objects can be Move resources, packages, or system objects.

#### Syntax

```
//# view-object <ID>
```

#### Options

```
<ID>: the ID of the object to be view.
```

#### Example

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

The `transfer-object` subcommand (`TransferObject` in Rust) is used to transfer ownership of an object from one account to another.

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

#### Example

```move
//# transfer-object 2,0 --sender acc1 --recipient acc2
```

`.exp` output:

```
task 3 'transfer-object'. lines 20-20:
mutated: object(0,0), object(2,0)
gas summary: computation_cost: 1000000, storage_cost: 2371200,  storage_rebate: 2371200, non_refundable_storage_fee: 0
```

### `consensus-commit-prologue`

The `consensus-commit-prologue` subcommand (`ConsensusCommitPrologue` in Rust) is used to commit a consensus event with a specific timestamp. It ensures that consensus-related operations maintain required order and timing.

#### Syntax

```
//# consensus-commit-prologue --timestamp-ms <<TIMESTAMP_MS>>
```

#### Options

```
-timestamp-ms <<TIMESTAMP_MS>>: specifies the timestamp (in milliseconds) at which the consensus event is committed. Commits a consensus event at the specific timestamp (which represents a specific moment in time in UTC).
```

Consensus commit prologue is available only in simulator mode.

#### Example

```move
//# init --addresses test=0x0 --accounts acc1 acc2 --protocol-version 1 --simulator

//# consensus-commit-prologue --timestamp-ms 4500
```

`.exp` output:

```
processed 3 tasks

init:
acc1: object(0,0), acc2: object(0,1)

task 1 'consensus-commit-prologue'. lines 3-3:
mutated: 0x0000000000000000000000000000000000000000000000000000000000000006
gas summary: computation_cost: 0, storage_cost: 0,  storage_rebate: 0, non_refundable_storage_fee: 0
```

### `programmable`

The `programmable` subcommand (`ProgrammableTransaction` in Rust) allows executing a programmable transaction with custom inputs, commands, and optional simulation mode. This subcommand provides control over transaction execution.

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

#### Example

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

Here, we're minting the `TestCoin` obj in fly and passing it to the `TransferObjects` PTB command via Result(0).

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

- `Input(u16)`: is an input argument, where the `u16` is the index of the input in the input vector. For example, given an input vector of `[Object1, Object2, Object3, Object4]`, `Object1` is accessed with `Input(0)` and `Object3` is accessed with `Input(2)`.
- `Gas`: is a special input argument representing the object for the `IOTA` coin used to pay for gas. It is kept separate from the other inputs because the gas coin is always present in each transaction and has special restrictions (you can only use it by-value with the `TransferObjects` command) that are not present for other inputs. Additionally, the gas coin being separate makes its usage explicit, which is helpful for sponsored transactions where the sponsor might not want the sender to use the gas coin for anything other than gas.
- `Result(u16)`: is an output of a command, that can be reused as input for another command. It is a special form of `NestedResult` where `Result(i)` is roughly equivalent to `NestedResult(i, 0)`. Unlike `NestedResult(i, 0)`, `Result(i)`, however, this errors if the result array at index `i` is empty or has more than one value. The ultimate intention of `Result` is to allow accessing the entire result array, but that is not yet supported. So in its current state, `NestedResult` can be used instead of `Result` in all circumstances.
- `NestedResult(u16,u16)`: uses the value from a previous command. The first `u16` is the index of the command in the command vector, and the second `u16` is the index of the result in the result vector of that command. For example, given a command vector of `[MoveCall1, MoveCall2, TransferObjects]` where `MoveCall2` has a result vector of `[Value1, Value2]`, `Value1` would be accessed with `NestedResult(1, 0)` and `Value2` would be accessed with `NestedResult(1, 1)`.

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

The `upgrade` subcommand (`UpgradePackage` in Rust) is used to upgrade an existing Move package on-chain. This allows for adding new features, fixing bugs, or optimizing performance while maintaining compatibility with previous versions.

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

#### Example

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

The `stage-package` subcommand (`StagePackage` in Rust) is used to prepare a package for future upgrades by staging it. Staging allows validation of the package's bytecode, dependencies, and structure before committing the upgrade. The package remains unpublished until explicitly upgraded.

#### Syntax

```
//# stage-package [OPTIONS]
```

#### Options

```
--syntax <SYNTAX>: specifies the syntax type (source or ir).
--dependencies <DEPENDENCIES> (optional): a list of package dependencies required for the staged package.
```

#### Example

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

The `set-address` subcommand (`SetAddress` in Rust) assigns a named address to an existing object, enabling it to be referenced by a human-readable identifier in subsequent commands. This is useful for improving readability and maintainability when working with objects in Move transactions.

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

#### Example

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

The `create-checkpoint` subcommand (`CreateCheckpoint` in Rust) forces the creation of one or more checkpoints in the system. A checkpoint represents a snapshot of the system state at a specific point in time. It is useful for maintaining consistency, enabling recovery, and improving performance in blockchain-based environments.

#### Syntax

```
//# create-checkpoint [COUNT]
```

#### Options

```
[COUNT]: specifies how many checkpoints to create. If omitted, a single checkpoint is created.
```

Checkpoints creation is available only in simulator mode.

#### Example

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

The `advance-epoch` subcommand (`AdvanceEpoch` in Rust) manually advances the epoch in the system. Epochs represent discrete time periods in a network, and transitioning to a new epoch can involve validator set changes, protocol upgrades, and other governance actions.

#### Syntax

```
//# advance-epoch [OPTIONS] [COUNT]
```

#### Options

```
[COUNT]: specifies the number of epochs to advance. If omitted, the command advances by one epoch.
--create-random-state: if set, generates a new random state when advancing the epoch.
```

#### Examples

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

The `advance-clock` subcommand (`AdvanceClock` in Rust) manually advances the system clock by a specified duration. This is useful for testing time-dependent features like transaction expiration, staking rewards, and scheduled events.

#### Syntax

```
//# advance-clock --duration-ns <DURATION_NS>
```

#### Options

```
--duration-ns <DURATION_NS>: specifies the duration (in nanoseconds) by which the clock should be advanced.
```

#### Example

```move
//# init --protocol-version 1 --simulator

//# create-checkpoint

// advance the clock by 1ms, next checkpoint timestamp should be 1970-01-01T00:00:00:001Z
//# advance-clock --duration-ns 1000000

//# create-checkpoint
```

`.exp` output:

```
processed 23 tasks

task 1 'create-checkpoint'. lines 3-5:
Checkpoint created: 1

task 3 'create-checkpoint'. lines 8-10:
Checkpoint created: 2
```

### `set-random-state`

The `set-random-state` subcommand (`SetRandomState` in Rust) sets the blockchain's random state for testing and development purposes. It allows specifying a randomness round, input bytes for randomness, and an initial version number for tracking.

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

#### Example

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

The `view-checkpoint` subcommand (`ViewCheckpoint` in Rust) retrieves and displays the latest checkpoint information from the blockchain. This is useful for debugging, monitoring, and ensuring data consistency across nodes.

#### Syntax

```
//# view-checkpoint
```

- Fetches the most recent checkpoint from the blockchain.
- Outputs details such as the checkpoint sequence number, epoch, digest, and gas info.

#### Example

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

#### Syntax

```
//# run-graphql [OPTIONS]
<GraphQL-query>
```

#### Options

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

### `bench`

The `bench` subcommand (`Bench` in Rust) is used to benchmark a specific transaction execution. This is particularly useful for measuring the performance of a Move function execution by running it under benchmarking conditions.

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
--syntax <SYNTAX>: defines the Move syntax type for transaction execution, either source (default) or IR.
```

#### Example

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

## How `run_test` Compares a Move File With the Corresponding `.exp` File

The `test_runner` compares `.move` files by executing them and comparing the output with an expected `.exp` files. This ensures that the Move program behaves as expected.

The main entry function for this process is `run_test_impl`.

```rust
pub async fn run_test_impl<'a, Adapter>(
    path: &Path,
    fully_compiled_program_opt: Option<Arc<FullyCompiledProgram>>,
) -> Result<(), Box<dyn std::error::Error>>
where
    Adapter: MoveTestAdapter<'a>,
    Adapter::ExtraInitArgs: Debug,
    Adapter::ExtraPublishArgs: Debug,
    Adapter::ExtraValueArgs: Debug,
    Adapter::ExtraRunArgs: Debug,
    Adapter::Subcommand: Debug,
{
    // Executes the .move file and captures output
    let output = handle_actual_output::<Adapter>(path, fully_compiled_program_opt).await?;

    // Compares actual output with expected .exp file
    handle_expected_output(path, output.0)?;

    Ok(())
}
```

### Execution Process in `handle_actual_output`

The `handle_actual_output` function is responsible for executing Move code and collecting the output by following these steps:

1. Initializing the Execution Environment.
   - The test adapter is initialized to set up the execution environment.
   ```rust
   let (mut adapter, result_opt) =
   Adapter::init(default_syntax, fully_compiled_program_opt, init_opt, path).await;
   ```
   - This prepares the necessary environment, including syntax options, precompiled programs, and initial state.

2. Parsing and Executing Commands from the `.move` File.
   - Reads the `.move` file.
   ```rust
   let mut tasks = taskify::<
       TaskCommand<
           Adapter::ExtraInitArgs,
           Adapter::ExtraPublishArgs,
           Adapter::ExtraValueArgs,
           Adapter::ExtraRunArgs,
           Adapter::Subcommand,
       >,
   >(path)?
   .into_iter()
   .collect::<VecDeque<_>>();
   assert!(!tasks.is_empty());
   ```
   - Converts recognized commands (e.g., `init`, `programmable`, `publish`) into structured execution tasks.
   - Ensures that the file contains at least one valid command.

3. Executing Each Task and Capturing the Output.
   - `handle_known_task` is responsible for executing parsed tasks from `.move` files based on its type (e.g., `init`, `programmable`, `publish`).

   ```rust
   for task in tasks {
       handle_known_task(&mut output, &mut adapter, task).await;
   }
   ```

   It uses `handle_command` to execute each command:

   - Init: initializes the test environment.
   - Run: calls a Move function.
   - PrintBytecode: compiled Move binary and prints its bytecode instructions.
   - Subcommand: handles other subcommands like `transfer-object`, `create-checkpoint`, etc.

   ```rust
   async fn handle_command(...) {
     match command {
               TaskCommand::Init { .. } => {
                   panic!("The 'init' command is optional. But if used, it must be the first command")
               }
               TaskCommand::Run(run_cmd, args) => { }
               TaskCommand::Publish(run_cmd, args) => { }
               TaskCommand::PrintBytecode(run_cmd, args) => { }
               TaskCommand::Subcommand(run_cmd, args) => { }
     }
   }
   ```

### Verification Process in `handle_expected_output`

1. Reading the expected output from the Corresponding `.exp` File
   - The `.exp` file contains expected execution results for comparison.

2. Comparing Actual and Expected Output

   - The function checks if the produced output matches the expected results.
   - If the output does not match, it computes the difference between expected and actual outputs and provides a mechanism for updating baselines if necessary.

   ```rust
   if output != expected_output {
           let msg = format!(
               "Expected errors differ from actual errors:\n{}",
               format_diff(expected_output, output),
           );
           anyhow::bail!(add_update_baseline_fix(msg))
       } else {
           Ok(())
       }
   ```

### Structure of the `.move` File.

A `.move` test file consists of commands and Move code, which are executed step by step. The structure follows these rules:

- Commands start with //#.
- Commands should be separated by an empty line, except when Move code is immediately following a specific command.
- The first command must be init.

Example of `.move` file structure:

```move
//# init --protocol-version 1 --addresses P0=0x0 --accounts A --simulator

// Split off a gas coin, so we have an object to query
//# programmable --sender A --inputs 1000 @A
//> SplitCoins(Gas, [Input(0)]);
//> TransferObjects([Result(0)], Input(1))

//# create-checkpoint

//# run-graphql
{
  sender: owner(address: "@{A}") {
    asObject { digest }
  }

  coin: owner(address: "@{obj_1_0}") {
    asObject { digest }
  }
}
```

### Structure of a `.exp` File

A `.exp` file contains the expected output for the `.move` test. It includes:

- A summary of processed tasks
- Execution results for each task
- Gas usage and storage fees (where applicable)
- GraphQL query responses (if applicable)
- The first line states the number of processed tasks.
- Each task output starts with task index, name, and line range.

Example of `.exp` file structure:

```exp
processed 4 tasks

init:
A: object(0,0)

task 1 'programmable'. lines 8-10:
created: object(1,0)
mutated: object(0,0)
gas summary: computation_cost: 1000000, storage_cost: 1976000,  storage_rebate: 0, non_refundable_storage_fee: 0

task 2 'create-checkpoint'. lines 12-12:
Checkpoint created: 1

task 3 'run-graphql'. lines 14-23:
Response: {
  "data": {
    "sender": {
      "asObject": null
    },
    "coin": {
      "asObject": {
        "digest": "4KjRv4dmBLHXtbw9LJJXeYfSoWeY8aFdkG7FooAqTZWq"
      }
    }
  }
}
```

It includes all 4 tasks execution with their output:

1. Init
2. Programmable
3. Create checkpoint
4. Run graphql

### Extending `handle_subcommand` and Creating New Subcommands

The `handle_subcommand` function is responsible for executing subcommands within the test framework. Each subcommand represents a specific action, such as executing Move calls, transferring objects, or publishing Move packages. If you need to extend `handle_subcommand` by adding a new subcommand, follow these steps:

#### 1. Define the New Subcommand in the Enum

New subcommands should be added to the `IotaSubcommand` enum, located inside the test adapter implementation:

```rust
#[derive(Debug)]
pub enum IotaSubcommand<ExtraValueArgs, ExtraRunArgs> {
    // Existing subcommands
    ViewObject(ViewObjectCommand),
    TransferObject(TransferObjectCommand),
    ProgrammableTransaction(ProgrammableTransactionCommand),
    ConsensusCommitPrologue(ConsensusCommitPrologueCommand),
    AdvanceEpoch(AdvanceEpochCommand),
    AdvanceClock(AdvanceClockCommand),
    CreateCheckpoint(CreateCheckpointCommand),
    SetAddress(SetAddressCommand),
    SetRandomState(SetRandomStateCommand),
    RunGraphql(RunGraphqlCommand),

    // New Subcommand
    CustomObjectAction(CustomObjectActionCommand),
}
```

### 2. Define the Command Struct

Each subcommand requires a struct that defines its arguments and expected input parameters. The struct should include:

- Named fields for each argument.
- #[derive(Debug)] for logging and debugging.

```rust
#[derive(Debug)]
pub struct CustomObjectActionCommand {
    pub target: String,  // Example argument
    pub value: u64,
    pub args: Vec<SomeArgs>
}
```

This struct will be parsed and used when executing the subcommand.

#### 3. Implement the Logic for the Subcommand

Modify the `handle_subcommand` function inside `IotaTestAdapter` to include the new subcommand's logic.

Locate the match statement inside `handle_subcommand`, and add your new subcommand:

```rust
async fn handle_subcommand(
    &mut self,
    task: TaskInput<Self::Subcommand>,
) -> anyhow::Result<Option<String>> {
    self.next_task();

    match command {
        // Other commands handling
        // ...

        // Custom subcommand implementation
        IotaSubcommand::CustomObjectAction(cmd) => {
            // Logic
        }
    }
}
```

#### 4. Add Tests Cases

Create `.move` and `.exp` files to test different scenarios.
