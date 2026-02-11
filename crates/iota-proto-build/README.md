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

## Selective Accessor Generation

iota-protoc-build supports selective accessor generation via custom proto field options.

Fields that need accessors are annotated in the `.proto` files using a custom option:

```protobuf
import "iota/grpc/options.proto";

message ObjectRequest {
  optional ObjectReference object_ref = 1 [(iota.grpc.generate_accessors) = "set,with"];
}
```

The `generate_accessors` option accepts a comma-separated list of accessor types:

**Individual Accessor Types:**

- `getter` - `field()` returns value or default (see limitations below)
- `getter_opt` - `field_opt()` returns `Option<&T>`
- `set` - `set_field()` setter method (mutable, modifies in place)
- `with` - `with_field()` builder-pattern setter (consumes self, returns modified self)
- `mut` - `field_mut()` returns `&mut T`
- `mut_opt` - `field_opt_mut()` returns `Option<&mut T>`

**Special Keywords:**

- `all` - Generates all accessor types (getter, getter_opt, set, with, mut, mut_opt)
  - **Note:** Cannot be combined with other accessor types
  - **Note:** Automatically includes default helpers when getter methods are generated
- `default` - Generates only `const_default()` and `default_instance()` helper functions
  - **Note:** Cannot be combined with `getter` or `all` (redundant, since getter includes defaults)
  - **Use case:** For fields that need default helpers but don't want getter methods (e.g., `"set,with,default"`)
