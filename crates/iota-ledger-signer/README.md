# iota-ledger-signer

High-level IOTA Ledger signer implementation for transaction signing and key management.

## Overview

This crate provides a convenient, high-level interface for using Ledger hardware wallets with the IOTA network. It wraps the lower-level `iota-ledger` crate and integrates with the IOTA SDK to provide seamless transaction signing and key management capabilities.

## Features

- **Simplified API**: High-level interface for common Ledger operations
- **IOTA SDK Integration**: Works seamlessly with `IotaClient` and IOTA transaction types
- **Key Management**: Easy access to public keys and addresses from derivation paths
- **Transaction Signing**: Sign transactions with automatic intent handling
- **Error Handling**: Comprehensive error types for robust applications

## Usage

```rust
use iota_ledger_signer::LedgerSigner;
use iota_sdk::IotaClient;
use bip32::DerivationPath;
use std::str::FromStr;

// Create a signer with default transport (HID or simulator)
let path = DerivationPath::from_str("m/44'/4218'/0'/0'/0'")?;
let client = Some(IotaClient::builder().build("https://api.testnet.iota.cafe").await?);
let signer = LedgerSigner::new_with_default(path, client)?;

// Get the signer's address
let address = signer.get_address()?;
println!("Signer address: {}", address);

// Get the public key
let public_key = signer.get_public_key()?;

// Sign a transaction
let signed_transaction = signer.sign_transaction(&transaction_data, &address).await?;
```

## Key Components

### LedgerSigner

The main struct that combines:

- A `Ledger` instance for hardware communication
- A BIP32 derivation path for key derivation
- An optional `IotaClient` for network operations

### Methods

- `get_address()`: Retrieve the IOTA address for the configured path
- `get_public_key()`: Get the public key for the configured path
- `sign_transaction()`: Sign transaction data with the Ledger device. This method automatically fetches all required objects referenced in the transaction from the network to enable clear-sign operation on the Ledger, allowing users to see readable transaction details on the device screen.
- `get_signature_scheme()`: Get the signature scheme (Ed25519)

## Error Handling

The crate defines `LedgerSignerError` which wraps:

- `iota_ledger::LedgerError`: Low-level Ledger communication errors
- `iota_sdk` errors: SDK-related errors
- Custom validation errors

## Integration with IOTA SDK

This crate is designed to work seamlessly with the IOTA SDK:

```rust
// Use with transaction builder
let tx_data = TransactionData::new_programmable(...);
let signed_tx = signer.sign_transaction(&tx_data, &sender_address).await?;
```
