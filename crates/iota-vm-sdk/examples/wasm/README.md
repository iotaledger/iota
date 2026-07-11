# Browser example

Runs transactions through the local Move VM compiled to wasm, in the browser,
and shows the result (status, gas, created/mutated/deleted objects, events).
Nothing is executed on a node; the node is only read from. Pick the network
(**testnet** / **mainnet** / **devnet** / **localnet**) at the top; each flow
then picks an execution mode — **dev-inspect** (relaxed checks, meters at the protocol max
gas) or **dry-run** (full validation at the transaction's own gas budget).

Two tabs:

- **Stake** builds a real `0x3::iota_system::request_add_stake` transaction with
  the IOTA TypeScript SDK and simulates it here — the decoded result includes
  the `StakingRequestEvent`. This mirrors `iota-rust-sdk`'s `stake.rs`, but
  where `stake.rs` dry-runs on the node, here the TS SDK only **builds** the
  transaction and the **wasm Move VM** runs it. It is built **gasless** — no gas
  payment — so the VM mints a mock gas coin the way a node's dev-inspect does,
  which is why it succeeds in both modes regardless of the sender's on-chain
  balance.
- **Your own transaction** decodes and simulates arbitrary BCS transaction
  bytes, optionally verifying supplied signatures. It ships example presets
  covering both happy and failing cases — a signed no-op, a sponsored
  transaction (sender + sponsor signatures), a valid and an invalid
  `MoveAuthenticator`, and a transaction that aborts in execution — so the
  result panel shows each succeeding or failing (and why). See
  [Example presets](#example-presets).

## On-demand object resolution

A transaction's bytes declare only its _input_ objects, but Move execution also
reads dynamic-field children — staking walks the validator set / staking pools
stored inside `IotaSystemState`, none of which are inputs, and an object can
have thousands of dynamic fields. So the JS side fetches nothing up front.
Instead the wasm store ([`CallbackStore`](../../src/wasm_store.rs)) resolves
objects **on demand**: whenever the VM reads an object it doesn't have, it calls
the JS `fetchObject(id, version)` callback, which fetches that object's BCS
from the selected network's GraphQL (at the exact version when given, else
latest) and returns it. The "Objects fetched on demand" panel logs each fetch.

Because the Move VM is synchronous, `fetchObject` uses a **synchronous**
`XMLHttpRequest`. That blocks the thread for the duration of the fetches; for a
non-blocking UI the simulation could be moved into a Web Worker (sync XHR is
fully supported there).

## Example presets

The "Your own transaction" tab loads its examples from
[`presets.json`](presets.json), rendered as chips. Each preset fills the
transaction and signature boxes and sets the recommended mode / signature
toggle:

| Preset              | What it shows                                           | Result                          |
| ------------------- | ------------------------------------------------------- | ------------------------------- |
| Signed no-op        | one signature over an empty transaction                 | body ✓, signature ✓             |
| Sender + sponsor    | a sponsored transaction with two signatures             | body ✓, both signatures ✓       |
| MoveAuthenticator ✓ | an on-chain Move `authenticate_ed25519` that accepts    | body ✓, signature ✓             |
| Invalid signature   | a valid transaction with a corrupted Ed25519 signature  | signature ✗, body not committed |
| MoveAuthenticator ✗ | the same Move authenticator rejecting a bogus signature | signature ✗, body not committed |
| Aborts in execution | splits more than the gas coin holds                     | signature ✓, body ✗             |

The result panel reports the transaction body and the signatures as two
separate verdicts. A rejected signature dominates: the transaction is
unauthorized, so its body does not take effect and is shown as "not committed"
rather than as a failure of its own logic — whether it is a standard signature
(checked before execution) or a `MoveAuthenticator` (Move code that aborts
mid-execution; skip the signature and the same body succeeds). Only when the
signature is accepted does the body's own outcome show — as in "Aborts in
execution", where a valid signature accompanies a body that fails on its own.

The two `MoveAuthenticator` presets are the crate's committed test fixtures
([`tests/fixtures/`](../../tests/fixtures)); they are **self-contained** —
each bundles every object the run reads and pins the protocol version, so it
runs offline regardless of the selected network. The other three are gasless
transactions built with the TS SDK and signed by fixed keypairs, so they run
against whichever network is selected.

`presets.json` is generated — regenerate it after changing an example or
refreshing the fixtures:

```sh
node gen-presets.mjs
```

## Run

Needs:

- the `wasm32-unknown-unknown` target and a `wasm-bindgen` CLI matching the
  crate's `wasm-bindgen` version (`cargo tree -p iota-vm-sdk -i wasm-bindgen`):

  ```sh
  rustup target add wasm32-unknown-unknown
  cargo install wasm-bindgen-cli --version <ver>   # e.g. 0.2.122
  ```

- Node.js + npm (to install `@iota/iota-sdk` and bundle the page with esbuild).

- a C compiler that targets wasm32, to build the C dependencies (e.g. `blst`,
  pulled in transitively via `fastcrypto`). The Linux/CI toolchains and most
  rustup-managed setups already have one. On macOS the system `clang` cannot
  target wasm32, so install LLVM and point `cc` at it:

  ```sh
  brew install llvm
  # in ~/.cargo/config.toml (machine-wide, not committed):
  #   [env]
  #   CC_wasm32_unknown_unknown = "/opt/homebrew/opt/llvm/bin/clang"
  #   AR_wasm32_unknown_unknown = "/opt/homebrew/opt/llvm/bin/llvm-ar"
  ```

Then one command builds the wasm, bundles the app, and serves it:

```sh
./serve.sh            # optional: ./serve.sh <port> (default 8000)
```

Open the printed <http://localhost:8000/>, pick a network and execution mode,
then **Simulate**. Pass `--rebuild` to force a wasm rebuild after changing the
crate (`./serve.sh --rebuild`).

## Notes

- The staking transaction is built **gasless**, so no private key is needed and
  its sender (`0xda18…2151`, from `stake.rs`) needs no coins on any network — it
  is only the declared sender. The VM mints a mock gas coin for it.
- A `not found` line in the fetch log is normal: the VM is probing for a
  dynamic-field entry that doesn't exist yet (a `dynamic_field`/`table`/`bag`
  existence check, which reads the child by its derived ID), and the Move code
  treats that absence as a valid result — so the simulation still succeeds.
