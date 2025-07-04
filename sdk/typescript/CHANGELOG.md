# @iota/iota-sdk

## 1.4.0

### Minor Changes

-   f04033d: Export Owner BCS type
-   ecea738: Improved logic around `fallbackMethods` in graphql-transport Introduced
    `unsupportedMethods` in graphql-transport Improved IotaClient compatibility with
    graphql-transport

### Patch Changes

-   59342b2: Renamed all instances of 'domain' to 'name' for IOTA-Names.
-   Updated dependencies [f04033d]
-   Updated dependencies [f04033d]
-   Updated dependencies [f04033d]
    -   @iota/bcs@1.1.0

## 1.3.0

### Minor Changes

-   6051799: Add support for passkeys
-   c837b79: Removed support for iota-bridge

### Patch Changes

-   5db9797: Add an util to trim addresses
-   c4c6d9a: Export `getGraphQLUrl` correctly

## 1.2.0

### Minor Changes

-   53d5058: Added iota names rpc methods to IotaClient and also GraphQL queries.

## 1.1.0

### Minor Changes

-   1128809: Add `getGraphQLUrl` shorthand to quickly get the graphql endpoint of the given network

### Patch Changes

-   acc502a: Add IOTA Names interfaces

## 1.0.1

### Patch Changes

-   26cf13b: Include mainnet into default network envs

## 1.0.0

### Major Changes

-   daa968f: Initial release of `@iota/bcs` and `@iota/iota-sdk`

### Minor Changes

-   864fd32: Rename `getLatestIotaSystemState` to `getLatestIotaSystemStateV1` and add a new
    backwards-compatible and future-proof `getLatestIotaSystemState` method that dynamically calls
    ``getLatestIotaSystemStateV1`or`getLatestIotaSystemStateV2` based on the protocol version of the
    node.

### Patch Changes

-   f4d75c7: Add graphql field in the network configuration.
-   Updated dependencies [daa968f]
    -   @iota/bcs@1.0.0

## 0.7.0

### Minor Changes

-   42898f1: Add support for getDynamicFieldObjectV2
-   bdb736e: Update clients after RPC updates to base64
-   65a0900: Add circulating supply support to the iota client

### Patch Changes

-   1ad39f9: Update dependencies

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
