# @iota/iota-sdk

## 1.10.1

### Patch Changes

-   1925bea: Bump with no changes to fix NPM versions

## 1.10.0

### Minor Changes

-   8e66840: Deprecate `AddressTransactionBlockRelationship.Sign`
-   b0d8be1: Support new `TransactionBlocksByDigests` GraphQL query.

### Patch Changes

-   4008cc6: Update validot to v1.2.0.

## 1.9.0

### Minor Changes

-   ae1385d: Update dependencies.

### Patch Changes

-   Updated dependencies [ae1385d]
    -   @iota/bcs@1.4.0

## 1.8.0

### Minor Changes

-   f4cc8e8: Add a `maxTransactionPayloadSize` service-configuration parameter for GraphQL schema
    introspection.
-   933496c: Changed type of "iotaTotalSupply" to BigInt in the GraphQL queries
-   0a4525d: Allow passing `rpId` to recover passkey accounts
-   ec99569: New subscription API types.

### Patch Changes

-   00f4a39: Fix some broken links in the README
-   Updated dependencies [147c97e]
    -   @iota/bcs@1.3.0

## 1.7.0

### Minor Changes

-   b152861: Fix the TS SDK bundling with Bun
-   a0b225d: Support the new node method `isTransactionIndexedOnNode`
-   fea81ab: Typo fixes.
-   4c68076: Sync with Node changes.
-   03e8b9b: Update move types
-   92dd15e: Add a new `waitMode` in `waitForTransaction`
-   2a5f065: Add support for the new `view` method in both JSON RPC and GraphQL transports.

### Patch Changes

-   c12d044: Internal simplification of the codegen
-   f3beb1e: Allow passing credential IDs to PasskeyKeypair methods

## 1.6.1

### Patch Changes

-   3244c29: Rename `parseIotaToNanos` util back to `parseAmount`

## 1.6.0

### Minor Changes

-   bd17ba9: Add support for `IotaMoveNormalizedEnum` type

### Patch Changes

-   c5543f0: Add suggestedGasPrice optional field to dryRunTransaction response
-   0accdb0: Add util to parse iota amount

## 1.5.0

### Minor Changes

-   61b0944: Added support for WrappedOrDeletedObject in TransactionBlockFilter
-   f008db3: Updated hex, base64, and base58 utility names for better consistency

    All existing methods will continue to work, but the following methods have been deprecated and
    replaced with methods with improved names:

    -   `toHEX` -> `toHex`
    -   `fromHEX` -> `fromHex`
    -   `toB64` -> `toBase64`
    -   `fromB64` -> `fromBase64`
    -   `toB58` -> `toBase58`
    -   `fromB58` -> `fromBase58`

-   5bbafa8: add deriveDynamicFieldID util
-   28ce666: Add new errors to ExecutionFailureStatus enum
-   f008db3: support Bech32 secrets in the Keypair.fromSecretKey methods
-   464c15a: Sync the APIs with the "Domain" -> "Name" rename of IotaNames

### Patch Changes

-   40576ed: Add balance formatting utils
-   966f83c: Add data to result of executeTransaction methods on Transaction executor classes
-   733df30: Add tx.object.option for creatnig object options in transaction builder
-   13ca264: Allow 0 amounts with `coinWithBalance` intent when the wallet has no coin objects of
    the required type.
-   c855f8c: Require name to register global transaction plugins
-   Updated dependencies [f008db3]
    -   @iota/bcs@1.2.0

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
