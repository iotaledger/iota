# Follow-up PR plan: one input-check path for execution and simulation

Follow-up to #12508 (unify dry run and dev inspect on `simulate_transaction`).
Addresses the root cause behind two defects found in review of that PR: the
simulation path validates its inputs through a parallel, hand-written function
that has silently drifted from the real one.

Branch off `develop` **after #12508 merges** (e.g.
`refactor/unify-simulation-input-checks`).

**This changes behavior** (see [Decisions to make](#decisions-to-make)) — it is
not a pure refactor, and it needs release notes.

## Why

`iota_transaction_checks` has two entry points that answer "may this transaction
run?":

- `check_transaction_input` (`crates/iota-transaction-checks/src/lib.rs:75`) —
  the real one, used for signing, certificates, and a dry run.
- `check_simulation_input` (`:176`) — used when the VM checks are disabled (dev
  inspect). It checks the transaction kind, rejects system transactions, and
  rejects a mutable object used twice. That is all.

The second was written as _"the minimum needed to make dev inspect run"_ rather
than derived as _"`check_transaction_input` minus the relaxations dev inspect
needs."_ Those two produce very different results: the first silently omits
whatever the author did not think about, and nothing notices later. It is also
inherited rather than designed here — the oldest commit touching it is the
initial Sui→IOTA rename (`0c44f89d2a`).

What that cost, both found in review of #12508:

- A gas payment naming two objects where one is not a `Coin<IOTA>` reached
  `GasCharger::smash_gas`, which states the input checks as a precondition and
  `panic!`s otherwise (`iota-execution/latest/iota-adapter/src/gas_charger.rs:163`).
  With `panic = 'abort'` on the release profile (`Cargo.toml:185`) that ends the
  process.
- A gas coin that could not cover the budget reached the adapter's
  `invariant_violation!` on the same precondition
  (`iota-execution/latest/iota-adapter/src/programmable_transactions/context.rs:201`).

#12508 patched both by adding `iota_types::gas::check_gas_coins_cover_budget`
and calling it from all three simulation paths. That fixes the instances, not
the class: the next check added to `check_transaction_input` will again not
apply to simulation, and nobody will be told.

One more finding from the same review to carry into this work:
`check_gas_coins_cover_budget` double-counts a gas coin named twice in the
payment, because it sums balances over the refs as given. Nothing wrong reaches
the engine today — every caller follows it with `check_simulation_input`, whose
`MutableObjectUsedMoreThanOnce` check rejects the duplicate — but the helper is
`pub` in `iota-types` and nothing on it says it relies on that ordering. Until
step 6 deletes it, its doc comment should state that it assumes the surrounding
input checks reject duplicate refs. Once its job returns to
`check_gas_balance`, the duplicate rejection lives in the same
`check_transaction_input` call and the assumption stops being load-bearing.

The codebase already knows about this hazard and handles it correctly one
function over. `check_certificate_input` (`:147`) carries this comment (`:141`):

> Since the purpose of this function is to audit certified transactions, the
> checks here should be a strict subset of the checks in
> `check_transaction_input()`. For checks not performed in this function but in
> `check_transaction_input()`, we should add a comment calling out the
> difference.

The certificate path got a stated subset relationship and a rule for
documenting divergence. The simulation path got neither.

## Inventory: what the real path enforces, and what a simulation needs relaxed

Taken from reading `check_transaction_input` and everything it calls. This is
the starting point for the work — each row becomes either "shared" or "named
relaxation".

| Check                                                       | Where                                            | Relax for simulation?                                                                                                                                                                                                 |
| ----------------------------------------------------------- | ------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| price ≥ reference gas price; price ≤ `max_gas_price`        | `IotaGasStatus::new` → `check_gas_preconditions` | **No** — already shared. Both paths build an `IotaGasStatus`, so both enforce it; `test_dev_inspect_gas_price` asserts `GasPriceUnderRGP`.                                                                            |
| budget ∈ [`min_transaction_cost`, `max_gas_budget`]         | `check_gas_balance` step 2                       | **Yes.** The one certain gas relaxation: an unsettled or deliberately small budget must run and report out-of-gas. `dev_inspect_meters_against_a_declared_budget` (`iota-vm-sdk`) depends on it.                      |
| gas objects are address-owned                               | `check_gas_balance` step 1                       | No                                                                                                                                                                                                                    |
| gas objects are `Coin<IOTA>`                                | `check_gas_balance` step 3 via `get_gas_balance` | No — omitting it was the panic                                                                                                                                                                                        |
| gas balance ≥ budget                                        | `check_gas_balance` step 3                       | No — omitting it was the invariant violation                                                                                                                                                                          |
| mutable object used more than once                          | `check_objects`                                  | No — the only check `check_simulation_input` kept                                                                                                                                                                     |
| non-genesis transaction has at least one input              | `check_objects`                                  | No                                                                                                                                                                                                                    |
| package used as object / object used as package             | `check_one_object`                               | No                                                                                                                                                                                                                    |
| input version < `Version::MAX_VALID_EXCL`                   | `check_one_object`                               | No — `test_dry_run_dev_inspect_max_gas_version` uses high but valid versions                                                                                                                                          |
| input object digest matches the loaded object               | `check_one_object`                               | **Undecided** — see below                                                                                                                                                                                             |
| input object owner == sender (gas: gas owner)               | `check_one_object`                               | **Yes** — `test_dev_inspect_unowned_object` asserts alice may use bob's object                                                                                                                                        |
| child object passed as owned; shared passed as owned        | `check_one_object`                               | Yes, same reason as owner                                                                                                                                                                                             |
| `Clock` as a mutable parameter outside a system transaction | `check_one_object`                               | No                                                                                                                                                                                                                    |
| receiving objects are current and disjoint from inputs      | `check_receiving_objects`                        | **Undecided** — `check_simulation_input` has a literal `TODO: check ReceivingObjects when simulating?`                                                                                                                |
| published packages pass the bytecode verifier               | `check_non_system_packages_to_be_published`      | No hard need — the adapter verifies during publish. Keep it shared unless it measurably costs; the signing-time limits differ from the adapter's, so skipping it makes a simulation _more_ permissive than execution. |

The required relaxation set is small: **budget bounds, input-object ownership**,
and the VM-level flags `VmChecks` already models. Everything else in the table
diverged for no recorded reason.

## Decisions to make

Both are behavior changes with user-visible effects. Decide before implementing;
each needs a release-note line.

1. **Input object digest.** A caller that knows an object's id and version but
   not its digest currently gets `InvalidObjectDigest` from a dry run and no
   check at all from a dev inspect. Sharing the check makes dev inspect stricter
   and could break callers who pass a zero or stale digest.
   _Recommendation:_ relax it for simulation, alongside ownership. A simulation
   answers "what would this do", and the digest is an optimistic-concurrency
   token for submission, not a semantic input. Note that a wholly nonexistent
   object still fails earlier in `read_objects_for_signing` —
   `test_dev_inspect_uses_unbound_object` asserts `ObjectNotFound` — so relaxing
   the digest does not let a simulation invent objects.
2. **Receiving objects.** Resolve the `TODO`. Sharing `check_receiving_objects`
   makes a dev inspect reject a stale receiving reference that it currently
   accepts. `iota-vm-sdk` already has coverage of both sides
   (`dev_inspect_skips_receiving_checks`, `dry_run_rejects_outdated_receiving_version`),
   so whichever way this goes, that test pair records the decision.
   _Recommendation:_ keep it relaxed for simulation and turn the `TODO` into a
   stated relaxation with a reason, so it stops reading as an oversight.

## Target shape

One function, with the relaxations named. A new type in `iota-types` next to
`VmChecks`, so `iota-transaction-checks` and every caller can name them:

```rust
// crates/iota-types/src/transaction_executor.rs

/// Which input checks a simulation drops relative to a transaction bound for
/// execution.
///
/// Every field defaults to `false`, so a check added to the shared path applies
/// to a simulation too until someone names it here and says why.
#[derive(Default, Debug, Copy, Clone)]
pub struct InputCheckRelaxations {
    /// Skip the bounds on the gas budget itself, so a caller whose gas is not
    /// settled runs out of gas rather than being rejected. The gas coins are
    /// still required to cover whatever budget is set.
    pub unbounded_gas_budget: bool,
    /// Skip the requirement that input objects be owned by the sender, so a
    /// caller can ask what a transaction would do over objects it does not own.
    ///
    /// This gates the whole `match object.owner` arm of `check_one_object`, not
    /// only the address mismatch: passing a child object or a shared object as
    /// an owned input is the same relaxation seen from a different owner
    /// variant, and gating only `Owner::Address` would leave a simulation
    /// rejecting those two while accepting a wrong address.
    pub any_object_owner: bool,
}

impl InputCheckRelaxations {
    /// No relaxations: exactly what a validator applies.
    pub const EXECUTION: Self = Self {
        unbounded_gas_budget: false,
        any_object_owner: false,
    };

    /// What a simulation with [`VmChecks::Disabled`] drops.
    pub const SIMULATION: Self = Self {
        unbounded_gas_budget: true,
        any_object_owner: true,
    };
}
```

Two fields, because the inventory found two certain relaxations. Decision 1, if
accepted, adds a third — `any_object_digest`, gating the `InvalidObjectDigest`
check in `check_one_object` and set in `SIMULATION` only. Decision 2, if it goes
the recommended way, adds no field: `check_receiving_objects` stays out of the
shared path and the existing `TODO` becomes a stated reason.

`check_transaction_input` gains a `relaxations: InputCheckRelaxations`
parameter and threads it to `check_gas` and `check_one_object`.
`check_simulation_input` is deleted; the three simulation call sites call
`check_transaction_input` with `InputCheckRelaxations::SIMULATION`.

`iota_types::gas::check_gas_coins_cover_budget` added in #12508 is deleted with
it — its job returns to `check_gas_balance`, which grows a
`bounded_budget: bool` parameter and skips only step 2 when it is false.

## Work, in order

Each step should build, test, and commit on its own.

1. **Add the type.** `InputCheckRelaxations` in
   `crates/iota-types/src/transaction_executor.rs`, with the two constants and
   the doc comments above. Nothing consumes it yet.
2. **Thread it through the gas check.** Give
   `IotaGasStatus::check_gas_balance` (`crates/iota-types/src/gas.rs:84`,
   forwarding to `crates/iota-types/src/gas_model/gas_v1.rs:306`) a
   `bounded_budget: bool` and gate step 2 on it. Pass `true` from `check_gas`
   for now, so behavior is unchanged. Prove it with a unit test that the same
   gas objects and a below-minimum budget are accepted with `false` and
   rejected with `true`.
3. **Thread it through the object check.** Give `check_objects` and
   `check_one_object` the `relaxations` parameter and gate the owner match (and
   the digest match, if decision 1 says so) on it. Pass
   `InputCheckRelaxations::EXECUTION` from `check_transaction_input_inner`, so
   behavior is unchanged.
4. **Open up `check_transaction_input`.** Add the `relaxations` parameter and
   forward it. Update the existing callers — signing and certificate paths — to
   pass `InputCheckRelaxations::EXECUTION` explicitly, so the call sites read as
   decisions rather than defaults. Behavior still unchanged; this is the commit
   a reviewer can check line by line.
5. **Move the simulation paths over.** Replace the `check_simulation_input` and
   `check_gas_coins_cover_budget` calls with a single
   `check_transaction_input(.., InputCheckRelaxations::SIMULATION, ..)` in:
   - `crates/iota-core/src/authority.rs:2284` and `:2290`
   - `crates/simulacrum/src/epoch_state.rs:259` and `:265`
   - `crates/iota-vm-sdk/src/executor/prepare.rs:175` and `:178`

   The three sites currently differ in error wrapping — `iota-vm-sdk` wraps in
   `ValidationError::new("simulation input check", ..)` — keep that.
6. **Delete the parallel path.** Remove `check_simulation_input`
   (`crates/iota-transaction-checks/src/lib.rs:176`) and
   `check_gas_coins_cover_budget` (`crates/iota-types/src/gas.rs`). The
   `WARNING!` doc comment on the former goes away with it: there is no longer a
   second function to warn about.
7. **Record the relaxations where they are chosen.** `InputCheckRelaxations::SIMULATION`
   is now the single place the divergence lives, and its doc comment is the
   explanation. Add the `check_certificate_input`-style note to
   `check_transaction_input` saying that a caller relaxing a check must name it
   in `InputCheckRelaxations` with a reason.

## Tests

The existing suite already pins most of this; the point of the work is that it
keeps passing.

- **Must keep passing unchanged**, these are the relaxations:
  `test_dev_inspect_unowned_object`, `test_dev_inspect_gas_price`,
  `test_dev_inspect_uses_unbound_object`,
  `test_dry_run_dev_inspect_max_gas_version` (`iota-core`);
  `dev_inspect_meters_against_a_declared_budget`,
  `dev_inspect_with_real_gas_coin_fills_in_a_zero_budget`,
  `dev_inspect_succeeds_with_zero_gas_budget`,
  `dev_inspect_succeeds_with_zero_gas_price` (`iota-vm-sdk`).
- **Must keep passing**, these are what the shared path buys —
  they were the #12508 fixes and should now hold by construction rather than by
  a separate call: `test_simulate_rejects_a_gas_payment_that_is_not_a_gas_coin`,
  `test_simulate_unset_gas_budget_uses_max_tx_gas` (`iota-core`);
  `dev_inspect_rejects_a_gas_payment_that_is_not_a_gas_coin`,
  `dev_inspect_rejects_a_gas_coin_that_cannot_back_the_budget` (`iota-vm-sdk`).
- **New**, one per row of the inventory that stops being relaxed. The cheapest
  form is a table-driven test over `[VmChecks::Enabled, VmChecks::Disabled]`
  asserting the same `UserInputError` from both, in
  `crates/iota-core/src/unit_tests/authority_tests.rs` next to the two #12508
  tests. At minimum: a package passed as an object, a child object passed as
  owned, and `Clock` as a mutable parameter.
- **New**, for each decision above once made: a test asserting the chosen
  behavior in both check modes, so the decision is recorded in code.
- 534 transactional tests across `iota-adapter-transactional-tests` and
  `iota-verifier-transactional-tests` with **zero `.exp` diffs** is the
  regression signal that steps 2–4 changed nothing; run them at the end of each
  of those steps, not only at the end.

## Out of scope

- The gas _fill-in_ rule (zero price → reference gas price, zero budget →
  `max_tx_gas`). #12508 already centralised that in the simulation entry points;
  it is a simulation concern and does not belong in a shared input check.
- `check_non_system_packages_to_be_published`. Sharing it is a no-op for
  correctness on the strict side and a possible latency change on a simulation
  that publishes; measure before touching it.
- Making `GasCharger::smash_gas` return an error instead of panicking. Worth
  doing — it is the unenforced precondition that made both defects possible, and
  a type-level guarantee would be better than a convention — but it is an
  execution-layer change with its own versioning constraints
  (`iota-execution/latest/`), so it needs its own plan.
