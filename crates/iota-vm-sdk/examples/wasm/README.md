# Browser example

Runs the `iota-vm-sdk` Move VM compiled to WebAssembly entirely in the browser.
It builds or accepts a transaction and simulates it against a live network —
objects are fetched on demand from GraphQL, nothing is executed on a node — then
shows the result.

What it enables:

- simulate a real `0x3::iota_system::request_add_stake` transaction built with
  the IOTA TypeScript SDK, or arbitrary BCS transaction bytes you provide;
- pick the network (testnet / mainnet / devnet / localnet) and the execution
  mode (`dev-inspect` or `dry-run`);
- verify signatures — including sponsored (sender + sponsor) and
  `MoveAuthenticator` transactions — with ready-made examples for the common
  success and failure cases.

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

Pass `--rebuild` to force a wasm rebuild after changing the crate. The "Your own
transaction" examples live in `presets.json`, regenerated with
`node gen-presets.mjs`.
