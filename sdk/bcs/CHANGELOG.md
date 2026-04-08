# @iota/bcs

## 1.6.0

### Minor Changes

-   1a2b37f: Use bcs.byteVector and bcs.bytes instead of bcs.vector(bcs.u8()) and bcs.fixedArray(n,
    bcs.u8()) to improve performance
-   045acb2: Add generic inference for bcs type names
-   bd1fa0b: Removed deprecated APIs. (toHEX, fromHEX, toB64, fromB64)
-   1a2b37f: improve perf of bcs.byteVector
-   045acb2: Update generics for vector, option, fixedArray, and map to prefer bcs types over input
    and output values
-   045acb2: Export new BcsStruct, BcsEnum and BcsTuple classes

### Patch Changes

-   adea573: Enable declarationMap in TS packages
-   c74bcc2: Fix encoding/decoding large uleb128 values Work around bug in turbopack thatbreaks
    uleb128 encoding in next@16
-   045acb2: Fix name in bcs error message

## 1.5.0

### Minor Changes

-   f445f37: Verify hex string is valid hex in `fromHex`
-   0cc417d: replace bs58 and bech32 packages with @scure/base

### Patch Changes

-   94b38e1: Accept arrays of numbers when serializing bcs.bytes()

## 1.4.0

### Minor Changes

-   ae1385d: Update dependencies.

## 1.3.0

### Minor Changes

-   147c97e: Update repository URLs.

## 1.2.0

### Minor Changes

-   f008db3: Updated hex, base64, and base58 utility names for better consistency

    All existing methods will continue to work, but the following methods have been deprecated and
    replaced with methods with improved names:

    -   `toHEX` -> `toHex`
    -   `fromHEX` -> `fromHex`
    -   `toB64` -> `toBase64`
    -   `fromB64` -> `fromBase64`
    -   `toB58` -> `toBase58`
    -   `fromB58` -> `fromBase58`

## 1.1.0

### Minor Changes

-   f04033d: Add bcs.byteVector for parsing a vector<u8> into a Uint8Array
-   f04033d: Allow BcsType.transform to omit input or output transform

### Patch Changes

-   f04033d: Correctly handle byteOffset in Uint8Arrays when reading bcs bytes

## 1.0.0

### Major Changes

-   daa968f: Initial release of `@iota/bcs` and `@iota/iota-sdk`

## 0.2.1

### Patch Changes

-   220fa7a: First public release.

## 0.2.0

### Minor Changes

-   6eabd18: Changes for compatibility with the node, simplification of exposed APIs and general
    improvements.

## 0.1.0

### Minor Changes

-   249a7d0: First release
