# Description of change

Please write a summary of your changes and why you made them.

## Links to any relevant issues

Be sure to reference any related issues by adding `fixes #(issue)`.

## How the change has been tested

Describe the tests that you ran to verify your changes.

Make sure to provide instructions for the maintainer as well as any relevant configurations.

- [ ] Basic tests (linting, compilation, formatting, unit/integration tests)
- [ ] Patch-specific tests (correctness, functionality coverage)

### Infrastructure QA (only required for crates that are maintained by @iotaledger/infrastructure)

- [ ] Synchronization of the indexer from genesis for a network including migration objects.
- [ ] Restart of indexer synchronization locally without resetting the database.
- [ ] Restart of indexer synchronization on a production-like database.
- [ ] Deployment of services using Docker.
- [ ] Verification of API backward compatibility.

## CI

Tick a box below to trigger the corresponding workflow on this PR's current HEAD.
Each box auto-unchecks once the run is dispatched — tick again to re-run.

- [ ] Run heavy tests (only changed crates) <!-- ci-trigger: heavy_tests.yml -->
- [ ] Run heavy tests (full workspace) <!-- ci-trigger: heavy_tests.yml test_only_changed_crates=false -->

### Release Notes

- [ ] Protocol:
- [ ] Nodes (Validators and Full nodes):
- [ ] Indexer:
- [ ] JSON-RPC:
- [ ] GraphQL:
- [ ] CLI:
- [ ] Rust SDK:
- [ ] gRPC:
