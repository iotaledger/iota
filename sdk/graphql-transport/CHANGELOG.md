# @iota/graphql-transport

## 0.5.1

### Patch Changes

-   Updated dependencies [26cf13b]
    -   @iota/iota-sdk@1.0.1

## 0.5.0

### Minor Changes

-   864fd32: Rename `getLatestIotaSystemState` to `getLatestIotaSystemStateV1` and add a new
    backwards-compatible and future-proof `getLatestIotaSystemState` method that dynamically calls
    ``getLatestIotaSystemStateV1`or`getLatestIotaSystemStateV2` based on the protocol version of the
    node.

### Patch Changes

-   f5d40a4: Added type mapping for consensus_gc_depth field of ProtocolConfig
-   Updated dependencies [f4d75c7]
-   Updated dependencies [daa968f]
-   Updated dependencies [864fd32]
    -   @iota/iota-sdk@1.0.0
    -   @iota/bcs@1.0.0

## 0.4.0

### Minor Changes

-   bdb736e: Update clients after RPC updates to base64

### Patch Changes

-   1ad39f9: Update dependencies
-   Updated dependencies [42898f1]
-   Updated dependencies [1ad39f9]
-   Updated dependencies [bdb736e]
-   Updated dependencies [65a0900]
    -   @iota/iota-sdk@0.7.0

## 0.3.0

### Minor Changes

-   1a4505b: Update clients to support committee selection protocol changes
-   e629a39: Aligns the Typescript SDK for the "fixed gas price" protocol changes:

    -   Add typing support for IotaChangeEpochV2 (computationCharge, computationChargeBurned).
    -   Add Typescript SDK client support for versioned IotaSystemStateSummary.

### Patch Changes

-   Updated dependencies [1a4505b]
-   Updated dependencies [e629a39]
-   Updated dependencies [2717145]
-   Updated dependencies [3fe0747]
-   Updated dependencies [e213517]
    -   @iota/iota-sdk@0.6.0

## 0.2.4

### Patch Changes

-   Updated dependencies [6e00091]
    -   @iota/iota-sdk@0.5.0

## 0.2.3

### Patch Changes

-   Updated dependencies [5214d28]
    -   @iota/iota-sdk@0.4.1

## 0.2.2

### Patch Changes

-   Updated dependencies [9864dcb]
    -   @iota/iota-sdk@0.4.0

## 0.2.1

### Patch Changes

-   220fa7a: First public release.
-   Updated dependencies [220fa7a]
    -   @iota/bcs@0.2.1
    -   @iota/iota-sdk@0.3.1

## 0.2.0

### Minor Changes

-   6eabd18: Changes for compatibility with the node, simplification of exposed APIs and general
    improvements.

### Patch Changes

-   Updated dependencies [6eabd18]
    -   @iota/bcs@0.2.0
    -   @iota/iota-sdk@0.3.0

## 0.1.2

### Patch Changes

-   d423314: Sync API changes:

    -   restore extended api metrics endpoints
    -   remove nameservice endpoints

-   b91a3d5: Update auto-generated files to latest IotaGenesisTransaction event updates
-   Updated dependencies [d423314]
-   Updated dependencies [b91a3d5]
-   Updated dependencies [a3c1937]
    -   @iota/iota-sdk@0.2.0

## 0.1.1

### Patch Changes

-   Updated dependencies [4a4ba5a]
    -   @iota/iota-sdk@0.1.1

## 0.1.0

### Minor Changes

-   249a7d0: First release

### Patch Changes

-   Updated dependencies [249a7d0]
    -   @iota/bcs@0.1.0
    -   @iota/iota-sdk@0.1.0
