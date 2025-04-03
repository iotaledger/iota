# @iota/iota-sdk

## 0.6.0

### Minor Changes

-   1a4505b: Update clients to support committee selection protocol changes
-   e629a39: Aligns the Typescript SDK for the "fixed gas price" protocol changes:

    -   Add typing support for IotaChangeEpochV2 (computationCharge, computationChargeBurned).
    -   Add Typescript SDK client support for versioned IotaSystemStateSummary.

-   2717145: Update `TransactionKind` and `TransactionKindIn` filter types from `string` to
    `IotaTransactionKind` type according to infra updates
-   e213517: Make `getChainIdentifier` use the Node RPC.

### Patch Changes

-   3fe0747: Enhance normalizeIotaAddress utility with optional validation

## 0.5.0

### Minor Changes

-   6e00091: Exposed maxSizeBytes in BuildTransactionOptions interface: Added the maxSizeBytes
    option to the BuildTransactionOptions interface to allow specifying the maximum size of the
    transaction in bytes during the build process.

## 0.4.1

### Patch Changes

-   5214d28: Update documentation urls

## 0.4.0

### Minor Changes

-   9864dcb: Add default royalty, kiosk lock, floor price & personal kiosk rules package ids to
    testnet network

## 0.3.1

### Patch Changes

-   220fa7a: First public release.
-   Updated dependencies [220fa7a]
    -   @iota/bcs@0.2.1

## 0.3.0

### Minor Changes

-   6eabd18: Changes for compatibility with the node, simplification of exposed APIs and general
    improvements.

### Patch Changes

-   Updated dependencies [6eabd18]
    -   @iota/bcs@0.2.0

## 0.2.0

### Minor Changes

-   a3c1937: Deprecate IOTA Name Service

### Patch Changes

-   d423314: Sync API changes:

    -   restore extended api metrics endpoints
    -   remove nameservice endpoints

-   b91a3d5: Update auto-generated files to latest IotaGenesisTransaction event updates

## 0.1.1

### Patch Changes

-   4a4ba5a: Make packages private

## 0.1.0

### Minor Changes

-   249a7d0: First release

### Patch Changes

-   Updated dependencies [249a7d0]
    -   @iota/bcs@0.1.0
