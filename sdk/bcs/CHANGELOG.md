# @iota/bcs

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
