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

### `transfer-object`

### `consensus-commit-prologue`

### `programmable`

### `upgrade`

### `stage-package`

### `set-address`

### `create-checkpoint`

### `advance-epoch`

### `advance-clock`

### `set-random-state`

### `view-checkpoint`

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

### `bench`

### `init`

The `init` command initializes the Move test environment. This command is used to set up various parameters such as named addresses, protocol versions, gas limits, and execution settings.

This command is **optional**, but if used, it must be the first command in the test sequence.

Command should be use:

- Before running any transactions in a test environment.
- When testing different protocol versions or gas pricing models.
- When working with named accounts and pre-defined addresses.
- For debugging storage behavior with object snapshots.

#### Syntax

```
//# init [OPTIONS]
```

Example:

```
//# init --accounts acc1 acc2 --addresses test=0x0 --protocol-version 1 --simulator
```

- Creates two accounts: acc1 and acc2.
- Uses protocol version 1.
- Map numerical address to the named representation in order to use named alias.
- Runs in simulator mode for controlled testing.

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
--object-snapshot-min-checkpoint-lag <OBJECT_SNAPSHOT_MIN_CHECKPOINT_LAG>: defines the minimum checkpoint lag for object snapshots. This affects when state snapshots are taken during execution
--object-snapshot-max-checkpoint-lag <OBJECT_SNAPSHOT_MAX_CHECKPOINT_LAG>: defines the maximum checkpoint lag for object snapshots
--flavor <FLAVOR>: Specifies the Move compiler flavor (e.g., Iota). 
The --flavor option in the init command specifies the Move language flavor that will be used in the environment. This option determines the syntax and semantics applied to Move programs and packages in the test adapter(Core or Iota).
--addresses <NAMED_ADDRESSES>: Maps custom named addresses to specific numerical addresses for the Move environment.
```

#### What is the simulator mode?

This type of execution when we can control the checkpoint, epoch creation process and manually advance clock as needed.
The simulator mode can be used when you need to debug shared objects or complex Move modules without waiting for full consensus validation.
You want full control over checkpointing and epochs for testing state transitions.

### `print-bytecode`

Command reads a compiled Move binary and prints its bytecode instructions in a readable format.

> Translates the given Move IR module into bytecode, then prints a textual
> representation of that bytecode

#### Syntax

```
//# print-bytecode
```

Example:

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

`Source` files have `.move` extension.
Represents a standard Move source code syntax.
`IR` files have `.mvir` extension. Represents a Move bytecode syntax, in order to debugging bytecode execution.

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

The publish command allows users to publish Move packages to the IOTA network. This command compiles the specified Move package and deploys it to the network, optionally marking it as upgradable.

#### Syntax

```
//# publish [OPTIONS]
```

Example:

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

- Publishes transfer.move on-chain.
- acc1 is the sender.
- The module is marked as upgradeable.
- Gas price is set to 1000.

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

Example:

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

`test::transfer` should have been published already before `run` command execution.

- Runs transfer function.
- acc1 is the sender.
- @acc2 is an identifier of recepient address.
- The gas price is set to 500.

Output:

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
