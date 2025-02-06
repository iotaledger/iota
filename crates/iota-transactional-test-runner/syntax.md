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

### `print-bytecode`

> Translates the given Move IR module into bytecode, then prints a textual
> representation of that bytecode

### `publish`

### `run`

## How `run_test` compares a Move File with the Corresponding .exp file

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

### **Execution Process in `handle_actual_output`**

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

### **Verification Process in `handle_expected_output`**

1. Reading the Expected Output from the Corresponding `.exp` File
   - The `.exp` file contains expected execution results for comparison.

2. Comparing Actual and Expected Output

   - The function checks if the produced output matches the expected results.
   - If the output does not match, it - computes the difference between expected and actual outputs and provides a mechanism for updating baselines if necessary.

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

### **Structure of the `.move` File.**

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

### **Structure of a `.exp` File**

A `.exp` file contains the expected output for the .move test. It includes:

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

### **Extending handle_subcommand and Creating New Subcommands**

The `handle_subcommand` function is responsible for executing subcommands within the test framework. Each subcommand represents a specific action, such as executing Move calls, transferring objects, or publishing Move packages. If you need to extend `handle_subcommand` by adding a new subcommand, follow these steps:

1. **Define the New Subcommand in the Enum**

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
    ForceObjectSnapshotCatchup(ForceObjectSnapshotCatchup),
    SetAddress(SetAddressCommand),
    SetRandomState(SetRandomStateCommand),
    RunGraphql(RunGraphqlCommand),

    // New Subcommand
    CustomObjectAction(CustomObjectActionCommand),
}
```

2. **Define the Command Struct**

Each subcommand requires a struct that defines its arguments and expected input parameters. The struct should include:

Named fields for each argument.
#[derive(Debug)] for logging and debugging.

```rust
#[derive(Debug)]
pub struct CustomObjectActionCommand {
    pub target: String,  // Example argument
    pub value: u64,
    pub args: Vec<SomeArgs>
}
```

This struct will be parsed and used when executing the subcommand.

3. **Implement the Logic for the Subcommand**

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

4. **Add a Test Case** - `.move` and `.exp` files to test different scenarios.
