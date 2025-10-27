# iota-ledger-signer

High-level IOTA Ledger signer implementation for transaction signing and key management.

## Overview

This crate provides a convenient, high-level interface for using Ledger hardware wallets with the IOTA network. It wraps the lower-level `iota-ledger` crate and integrates with the IOTA SDK to provide seamless transaction signing and key management capabilities.

## Examples

This crate provides a sample transaction signing implementation in `examples/ledger_signer.rs`.
To run the example, use:

```bash
cargo run --example ledger_signer -- --path "m/44'/4218'/0'/0'/0'" --network testnet --tx "<base64 encoded tx>"
```
