# How to generate new protocol config snapshot files

IMPORTANT: never update snapshots. only add new versions!

## Prerequisites

cargo install cargo-insta

## Generate the new protocol config snapshot files

Run `cargo test -p iota-protocol-config` to generate a new protocol config snapshot.
The generated file will have the `.new` extension. Then run `cargo insta review` and accept
the changes to finalize the snapshot file (it will remove the `.new` suffix).

Repeat this process until all snapshot files for this version were created and the test doesn't fail anymore.
