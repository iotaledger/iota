# @iota/graphql-transport

## 0.15.0

### Minor Changes

-   0be1d8d: Add a new RequestInspector API to look into requests and responses of an IotaClient

### Patch Changes

-   331b7ab: Refreshed the README with up to date APIs, examples and more.
-   adea573: Enable declarationMap in TS packages
-   1359874: Add fallback values to getBalance call if there is no balance for the requested
    address.
-   f116b86: fix undefined exchangeRatesId and stakingPoolId
-   Updated dependencies [fe97265]
-   Updated dependencies [d2ed95a]
-   Updated dependencies [379dd4b]
-   Updated dependencies [1a2b37f]
-   Updated dependencies [1a2b37f]
-   Updated dependencies [66d0c0d]
-   Updated dependencies [9c6137c]
-   Updated dependencies [331b7ab]
-   Updated dependencies [adea573]
-   Updated dependencies [bd1fa0b]
-   Updated dependencies [1a2b37f]
-   Updated dependencies [3f80310]
-   Updated dependencies [2a9a1df]
-   Updated dependencies [c74bcc2]
-   Updated dependencies [045acb2]
-   Updated dependencies [999224b]
-   Updated dependencies [1a2b37f]
-   Updated dependencies [f3d9079]
-   Updated dependencies [7849d0e]
-   Updated dependencies [045acb2]
-   Updated dependencies [bd1fa0b]
-   Updated dependencies [1a2b37f]
-   Updated dependencies [045acb2]
-   Updated dependencies [0be1d8d]
-   Updated dependencies [235a8d7]
-   Updated dependencies [46171b1]
-   Updated dependencies [9d88461]
-   Updated dependencies [045acb2]
    -   @iota/iota-sdk@1.12.0
    -   @iota/bcs@1.6.0

## 0.14.0

### Minor Changes

-   19c174f: Add enum pagination support to getNormalizedMoveModule and fix type generator for
    arrays without items
-   8bd4574: Sync types with the new score integration.
-   d5923e9: Add the `MoveAuthenticator` variant to the `GenericSignature` to allow the
    authentication of Abstract Accounts.
-   ffbb296: Update the GraphQL queries to support new fields
-   f22df28: Change the thrown error of `getObject` when using the GraphQL transport for it to
    behave like the JSON-RPC transport.

### Patch Changes

-   54c7803: Update @types/node to v24.
-   f2b8160: fix graphql transport error
-   abcdd2f: add transactionBlocksByDigests tests
-   6f06e01: Bring back unsupportedFilters for `getOwnedObjects`
-   Updated dependencies [43cfa2b]
-   Updated dependencies [94b38e1]
-   Updated dependencies [3bcb711]
-   Updated dependencies [0296e7d]
-   Updated dependencies [54c7803]
-   Updated dependencies [b903c0a]
-   Updated dependencies [f5a4569]
-   Updated dependencies [19c174f]
-   Updated dependencies [8bd4574]
-   Updated dependencies [5fc7e20]
-   Updated dependencies [7a61cb5]
-   Updated dependencies [f2b8160]
-   Updated dependencies [abcdd2f]
-   Updated dependencies [d5923e9]
-   Updated dependencies [6fc20db]
-   Updated dependencies [b72bfd2]
-   Updated dependencies [38657f6]
-   Updated dependencies [7fa1fde]
-   Updated dependencies [2164846]
-   Updated dependencies [4dc4b84]
-   Updated dependencies [58891a9]
-   Updated dependencies [f445f37]
-   Updated dependencies [924bf18]
-   Updated dependencies [21a4820]
-   Updated dependencies [ffbb296]
-   Updated dependencies [088c577]
-   Updated dependencies [0cc417d]
    -   @iota/iota-sdk@1.11.0
    -   @iota/bcs@1.5.0

## 0.13.1

### Patch Changes

-   Updated dependencies [1925bea]
    -   @iota/iota-sdk@1.10.1

## 0.13.0

### Minor Changes

-   8e66840: Deprecate `AddressTransactionBlockRelationship.Sign`
-   b0d8be1: Support new `TransactionBlocksByDigests` GraphQL query.

### Patch Changes

-   Updated dependencies [4008cc6]
-   Updated dependencies [8e66840]
-   Updated dependencies [b0d8be1]
    -   @iota/iota-sdk@1.10.0

## 0.12.0

### Minor Changes

-   ae1385d: Update dependencies.

### Patch Changes

-   Updated dependencies [ae1385d]
    -   @iota/bcs@1.4.0
    -   @iota/iota-sdk@1.9.0

## 0.11.0

### Minor Changes

-   f4cc8e8: Add a `maxTransactionPayloadSize` service-configuration parameter for GraphQL schema
    introspection.
-   933496c: Changed type of "iotaTotalSupply" to BigInt in the GraphQL queries
-   147c97e: Update repository URLs.
-   ec99569: New subscription API types.

### Patch Changes

-   Updated dependencies [00f4a39]
-   Updated dependencies [f4cc8e8]
-   Updated dependencies [933496c]
-   Updated dependencies [147c97e]
-   Updated dependencies [0a4525d]
-   Updated dependencies [ec99569]
    -   @iota/iota-sdk@1.8.0
    -   @iota/bcs@1.3.0

## 0.10.0

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
-   58997cb: map Option correctly in moveDataToRpcContent
-   Updated dependencies [b152861]
-   Updated dependencies [c12d044]
-   Updated dependencies [f3beb1e]
-   Updated dependencies [a0b225d]
-   Updated dependencies [fea81ab]
-   Updated dependencies [4c68076]
-   Updated dependencies [03e8b9b]
-   Updated dependencies [92dd15e]
-   Updated dependencies [2a5f065]
    -   @iota/iota-sdk@1.7.0

## 0.9.2

### Patch Changes

-   Updated dependencies [3244c29]
    -   @iota/iota-sdk@1.6.1

## 0.9.1

### Patch Changes

-   Updated dependencies [c5543f0]
-   Updated dependencies [bd17ba9]
-   Updated dependencies [0accdb0]
    -   @iota/iota-sdk@1.6.0

## 0.9.0

### Minor Changes

-   61b0944: Added support for WrappedOrDeletedObject in TransactionBlockFilter
-   464c15a: Sync the APIs with the "Domain" -> "Name" rename of IotaNames

### Patch Changes

-   Updated dependencies [40576ed]
-   Updated dependencies [61b0944]
-   Updated dependencies [966f83c]
-   Updated dependencies [f008db3]
-   Updated dependencies [733df30]
-   Updated dependencies [13ca264]
-   Updated dependencies [5bbafa8]
-   Updated dependencies [28ce666]
-   Updated dependencies [c855f8c]
-   Updated dependencies [f008db3]
-   Updated dependencies [464c15a]
    -   @iota/iota-sdk@1.5.0
    -   @iota/bcs@1.2.0

## 0.8.0

### Minor Changes

-   ecea738: Improved logic around `fallbackMethods` in graphql-transport Introduced
    `unsupportedMethods` in graphql-transport Improved IotaClient compatibility with
    graphql-transport

### Patch Changes

-   ecea738: Added missing GraphQL query option fields.
-   59342b2: Renamed all instances of 'domain' to 'name' for IOTA-Names.
-   Updated dependencies [f04033d]
-   Updated dependencies [f04033d]
-   Updated dependencies [59342b2]
-   Updated dependencies [f04033d]
-   Updated dependencies [f04033d]
-   Updated dependencies [ecea738]
    -   @iota/iota-sdk@1.4.0
    -   @iota/bcs@1.1.0

## 0.7.0

### Minor Changes

-   c837b79: Removed support for iota-bridge

### Patch Changes

-   Updated dependencies [6051799]
-   Updated dependencies [5db9797]
-   Updated dependencies [c4c6d9a]
-   Updated dependencies [c837b79]
    -   @iota/iota-sdk@1.3.0

## 0.6.0

### Minor Changes

-   53d5058: Added iota names rpc methods to IotaClient and also GraphQL queries.

### Patch Changes

-   Updated dependencies [53d5058]
    -   @iota/iota-sdk@1.2.0

## 0.5.2

### Patch Changes

-   Updated dependencies [acc502a]
-   Updated dependencies [1128809]
    -   @iota/iota-sdk@1.1.0

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
