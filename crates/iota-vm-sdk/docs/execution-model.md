# Execution model: phases, gas budgets, and SDK modes

This crate runs transactions against the node's Move engine offline. To keep
results faithful, each SDK path mirrors a specific node phase and meters gas the
same way. This document maps the two together.

## Node phases (reference)

The node runs a transaction in two phases, and the gas budget depends on whether
`MoveAuthenticator`s are present. `max_auth_gas` is a protocol-config value; it
**caps** the budget only in the pre-consensus signing phase. Post-consensus it
is passed only to enforce the "Move authentication is enabled" precondition — it
does not cap the budget there.

| Node phase                                     | Check function                                   | budget arg     | `is_execute_...` | resulting `IotaGasStatus` budget | engine call                                        | who runs                                                                                                  |
| ---------------------------------------------- | ------------------------------------------------ | -------------- | ---------------- | -------------------------------- | -------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Pre-consensus signing (with authenticators)    | `check_transaction_input`                        | `max_auth_gas` | `false`          | **`max_auth_gas`** (capped)      | `authenticate_transaction`                         | filtered set — sponsor-only if sponsored and the flag is on, else all — sharing one `max_auth_gas` budget |
| Post-consensus cert exec (with authenticators) | `check_certificate_and_move_authenticator_input` | `max_auth_gas` | `true`           | **full tx budget**               | `authenticate_then_execute_transaction_to_effects` | all authenticators + body share the full tx budget                                                        |
| Post-consensus cert exec (no authenticators)   | `check_certificate_input`                        | `0`            | `true`           | **full tx budget**               | `execute_transaction_to_effects`                   | body only                                                                                                 |

The gas-coin **balance** is always checked against the full transaction budget,
in every phase.

## SDK modes

`ExecutionMode` selects input-check relaxation, the gas budget, and whether
effects are committed:

| Mode         | Input check                                                                         | Gas price                                   | Gas budget                                                    | Mock gas coin if none supplied? | Commits to store? |
| ------------ | ----------------------------------------------------------------------------------- | ------------------------------------------- | ------------------------------------------------------------- | ------------------------------- | ----------------- |
| `DevInspect` | `check_simulation_input` (relaxed), plus a gas balance check                        | declared, or the reference gas price if `0` | declared, or the coins' balance capped at `max_tx_gas` if `0` | yes                             | no                |
| `DryRun`     | `check_transaction_input` (authenticator budget `0` → meters at the full tx budget) | declared, or the reference gas price if `0` | declared, or the coins' balance capped at `max_tx_gas` if `0` | yes                             | no                |
| `Execute`    | `check_transaction_input` (authenticator budget `0` → meters at the full tx budget) | declared                                    | declared                                                      | no — requires real gas          | yes, on success   |

Both simulation modes fill in gas the caller left unset, the same way the node's
simulation paths do: a zero price is below the reference gas price and a zero
budget cannot cover any computation, so neither is a value to meter against, and
a run whose gas is not yet settled is the common case. `Execute` commits its
effects, so it holds a transaction to its own declared gas the way a validator
would.

Whatever the budget resolves to, the gas coins have to cover it — the engine
smashes the whole budget off them before running any command. `DryRun` gets that
check from `check_transaction_input`; `DevInspect` skips that check and so
carries its own, rejecting an under-funded coin with `GasBalanceTooLow` rather
than letting the engine hit an invariant violation. A zero budget is capped at
what the coins hold, so estimating does not require holding `max_tx_gas` — but
the budget is still held back for the whole programmable transaction, so a
transaction that also pays out of its gas coin has to declare a budget leaving
room for that.

`DryRun` and `Execute` are otherwise identical in preparation and budget; they
differ only in the mock-gas rule and whether effects are committed.

## SDK entry points and the phase each mirrors

| Entry point                                 | Signatures                           | Engine call                                                      | Gas budget                                | Mirrors node phase                 | `SignatureStatus`                                                             |
| ------------------------------------------- | ------------------------------------ | ---------------------------------------------------------------- | ----------------------------------------- | ---------------------------------- | ----------------------------------------------------------------------------- |
| `execute`                                   | none checked                         | `dev_inspect_transaction` (`dev_inspect` = mode is `DevInspect`) | per mode (see above)                      | post-consensus body-only execution | `NotChecked`                                                                  |
| `execute_signed`, standard schemes          | verified cryptographically           | `dev_inspect_transaction`                                        | per mode                                  | post-consensus body-only execution | `Verified`                                                                    |
| `execute_signed`, with `MoveAuthenticator`s | crypto + authenticator run in the VM | `authenticate_then_execute_transaction_to_effects`               | per mode, shared by authenticators + body | post-consensus certified execution | `Verified`, or `Failed` if an authenticator rejects                           |
| `check_signing_authentication`              | crypto + authenticator run in the VM | `authenticate_transaction` (no body, commits nothing)            | **`max_auth_gas`**                        | pre-consensus signing              | `Verified`, or `Failed` if an authenticator rejects or exceeds `max_auth_gas` |

### Authenticator outcome

The authenticator path runs the authenticators and body together to effects. On
failure the cause is ambiguous, so:

- Failure in a command **after** the authenticators' fake command 0 → a body
  abort, never a rejection. The authenticators passed; no re-run.
- Failure in command 0 or unattributed → re-run the authenticators alone (via
  `authenticate_transaction`) to tell a rejection from a body abort. The re-run
  meters at the same budget the combined run used (the full tx budget outside
  `DevInspect`, matching post-consensus), so it never reports a rejection for a
  run the combined execution had enough gas for.

A protocol version that predates Move authentication (no `max_auth_gas`) is
rejected up front with `UnsupportedProtocolVersion` rather than reaching the
engine.

## Modelling both phases

`execute_signed` and `check_signing_authentication` model the two node phases
separately:

- `execute_signed` runs the authenticators and body to effects under the
  per-mode budget, never capped at `max_auth_gas` — the **post-consensus**
  path. On its own it accepts an authenticator that would exceed `max_auth_gas`
  at signing.
- `check_signing_authentication` runs only the pre-consensus authenticator set
  under `max_auth_gas` — the **pre-consensus** signing check. It produces no
  effects; use it to tell whether a validator would admit the transaction for
  signing.

A transaction is accepted on-chain only if it passes both, so a caller that
needs the full picture runs both. Neither models the deny-list or input policies
a validator also applies (see the deny-list check the node runs before the
authenticators).
