# To build the `iota-framework` packages and include them in the latest protocol version snapshot during the development phase

1. Edit any of the modules in `./packages` and check their correctness with the command `iota move test`.
2. Then, (in `crates/iota-framework`) run `UPDATE=1 cargo insta test`. This updates the packages build in `./packages_compiled`.
3. Finally, (in the root folder) run `cargo run --release --bin iota-framework-snapshot`. This updates the packages snapshot for the current (working) protocol version.

# To add a new native Move function

1. Add a new `./iota-framework/{name}.move` file or find an appropriate `.move`.
2. Add the signature of the function you are adding in `{name}.move`.
3. Add the rust implementation of the function under `./iota-framework/src/natives` with name `{name}.rs`.
4. Link the move interface with the native function in [all_natives](https://github.com/iotaledger/iota/tree/develop/iota-execution/latest/iota-move-natives/src)
5. Write some tests in `{name}_tests.move` and pass `run_framework_move_unit_tests`.
6. Optionally, run `cargo insta test` and `cargo insta review` since the iota-framework build will change the empty genesis config.

Note: The gas metering for native functions is currently a WIP; use a dummy value for now and please open an issue with `move` label.
