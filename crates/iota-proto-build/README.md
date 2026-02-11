# iota-proto-build

Code generation tool for IOTA gRPC protocol buffers.

## Purpose

This tool generates Rust code from `.proto` files with additional field masking support. It creates:

- Standard prost/tonic gRPC types
- Field constants and `MessageFields` trait implementations
- Field path builders for constructing field masks

## Usage

Run this tool whenever you modify `.proto` files.

```bash
cd crates/iota-grpc-types
make proto
```

**NOTE**: After generating files, the tool checks if any generated code changed. If changes are detected, you must commit them before running the tool again. This ensures generated code is never forgotten and stays in sync with proto definitions.

## Output

Generated files are written to `crates/iota-grpc-types/src/proto/generated/`:

- `iota.grpc.v0.*.rs` - Standard protobuf types
- `iota.grpc.v0.*.field_info.rs` - Field metadata and builders

**Important**: Commit the generated files to git. They are part of the source code, not build artifacts.

## When to Regenerate

- After adding or modifying `.proto` files
- After changing message structures or fields
- After updating proto dependencies

## Proto Files Location

Source proto files: `crates/iota-grpc-types/proto/iota/grpc/v0/`
