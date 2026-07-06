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

| Mode         | Input check                                             | Gas budget                                                            | Mock gas coin if none supplied? | Commits to store? |
| ------------ | ------------------------------------------------------- | --------------------------------------------------------------------- | ------------------------------- | ----------------- |
| `DevInspect` | `check_dev_inspect_input` (relaxed)                     | `max_tx_gas` (mock gas) or `min(max_tx_gas, coin balance)` (real gas) | yes                             | no                |
| `DryRun`     | `check_transaction_input` (budget `0` → full tx budget) | transaction's declared budget                                         | yes                             | no                |
| `Execute`    | `check_transaction_input` (budget `0` → full tx budget) | transaction's declared budget                                         | no — requires real gas          | yes, on success   |

Dev-inspect meters at `max_tx_gas` rather than the declared budget because a run
before a budget is settled isn't limited by one; a real gas coin still caps it at
the coin's balance. `DryRun` and `Execute` use the same preparation and budget;
they differ only in the mock-gas rule and whether effects are committed.

## SDK entry points and the phase each mirrors

| Entry point                                 | Signatures                           | Engine call                                                      | Gas budget           | Mirrors node phase                 | `SignatureStatus`                                   |
| ------------------------------------------- | ------------------------------------ | ---------------------------------------------------------------- | -------------------- | ---------------------------------- | --------------------------------------------------- |
| `execute`                                   | none checked                         | `dev_inspect_transaction` (`dev_inspect` = mode is `DevInspect`) | per mode (see above) | post-consensus body-only execution | `NotChecked`                                        |
| `execute_signed`, standard schemes          | verified cryptographically           | `dev_inspect_transaction`                                        | per mode             | post-consensus body-only execution | `Verified`                                          |
| `execute_signed`, with `MoveAuthenticator`s | crypto + authenticator run in the VM | `authenticate_then_execute_transaction_to_effects`               | **full tx budget**   | post-consensus certified execution | `Verified`, or `Failed` if an authenticator rejects |

### Authenticator verdict

The authenticator path runs the authenticators and body together to effects. On
failure the cause is ambiguous, so:

- Failure in a command **after** the authenticators' fake command 0 → a body
  abort, never a rejection. The authenticators passed; no re-run.
- Failure in command 0 or unattributed → re-run the authenticators alone (via
  `authenticate_transaction`) to tell a rejection from a body abort. The re-run
  meters at the **full tx budget**, matching post-consensus, so it never reports
  a rejection for a run the real execution had enough gas for.

A protocol version that predates Move authentication (no `max_auth_gas`) is
rejected up front with `UnsupportedProtocolVersion` rather than reaching the
engine.

## What the SDK does not model

The pre-consensus signing phase — the `max_auth_gas`-capped authenticator check a
validator runs before certifying. `execute_signed` models post-consensus
execution only, so an authenticator that would exceed `max_auth_gas` at signing
but fits within the transaction budget is reported as `Verified`.
