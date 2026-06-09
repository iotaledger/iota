# Browser example

Builds a real `0x3::iota_system::request_add_stake` transaction with the IOTA
TypeScript SDK against **testnet**, then simulates it in the local Move VM
compiled to wasm — and shows the result (status, gas, created/mutated/deleted
objects, and the decoded `StakingRequestEvent`). Nothing is executed on a node;
the node is only read from.

This mirrors `iota-rust-sdk`'s `stake.rs` example, but where `stake.rs`
dry-runs on the node, here the TS SDK only **builds** the transaction and the
**wasm Move VM** runs it.

## On-demand object resolution

A transaction's bytes declare only its _input_ objects, but Move execution also
reads dynamic-field children — staking walks the validator set / staking pools
stored inside `IotaSystemState`, none of which are inputs, and an object can
have thousands of dynamic fields. So the JS side fetches nothing up front.
Instead the wasm store ([`CallbackStore`](../../src/wasm_store.rs)) resolves
objects **on demand**: whenever the VM reads an object it doesn't have, it calls
the JS `fetchObject(id)` callback, which fetches that object's BCS from testnet
GraphQL and returns it. The "Objects fetched on demand" panel logs each fetch.

Because the Move VM is synchronous, `fetchObject` uses a **synchronous**
`XMLHttpRequest`. That blocks the thread for the duration of the fetches; for a
non-blocking UI the simulation could be moved into a Web Worker (sync XHR is
fully supported there).

## Run

Needs:

- the `wasm32-unknown-unknown` target and a `wasm-bindgen` CLI matching the
  crate's `wasm-bindgen` version (`cargo tree -p iota-vm-sdk -i wasm-bindgen`):

  ```sh
  rustup target add wasm32-unknown-unknown
  cargo install wasm-bindgen-cli --version <ver>   # e.g. 0.2.122
  ```

- Node.js + npm (to install `@iota/iota-sdk` and bundle the page with esbuild).

Then one command builds the wasm, bundles the app, and serves it:

```sh
./serve.sh            # optional: ./serve.sh <port> (default 8000)
```

Open the printed <http://localhost:8000/> and click **Simulate stake on
testnet**. Pass `--rebuild` to force a wasm rebuild after changing the crate
(`./serve.sh --rebuild`).

## Notes

- Simulation is unsigned (dev-inspect), so no private key is needed. The sender
  (`0xda18…2151`, from `stake.rs`) only has to own a coin the SDK can pick for
  gas; if testnet has been reset, point `SENDER` in `app.ts` at any funded
  testnet address.
- A `not found` line in the fetch log is normal: the VM is probing for a
  dynamic-field entry that doesn't exist yet (a `dynamic_field`/`table`/`bag`
  existence check, which reads the child by its derived ID), and the Move code
  treats that absence as a valid result — so the simulation still succeeds.
