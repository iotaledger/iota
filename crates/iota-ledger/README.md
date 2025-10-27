# iota-ledger

Low-level IOTA Ledger hardware wallet integration library.

## Overview

This crate provides direct communication with IOTA Ledger applications running on Ledger hardware wallets or the Speculos simulator. It implements the APDU protocol for key operations like getting public keys, signing transactions, and managing the app lifecycle.

## Features

- **Multiple Transport Support**: Connects via USB HID or TCP (for Speculos simulator)
- **Key Management**: Retrieve public keys and IOTA addresses using BIP32 derivation paths
- **Transaction Signing**: Sign IOTA transactions with hardware wallet security
- **App Management**: Open/close IOTA app and check app status
- **Address Verification**: Display and verify addresses on device screen
- **Version Information**: Query the IOTA app version on the device

## Usage

```rust
use iota_ledger::Ledger;
use bip32::DerivationPath;
use std::str::FromStr;

// Create a ledger instance (automatically detects HID or simulator)
let ledger = Ledger::new_with_default()?;

// Or explicitly use the simulator
let ledger = Ledger::new_with_simulator()?;

// Define a BIP32 derivation path
let path = DerivationPath::from_str("m/44'/4218'/0'/0'/0'")?;

// Get public key and address
let public_key_result = ledger.get_public_key(&path)?;
println!("Address: {}", public_key_result.address);

// Verify address on device (shows on screen for user confirmation)
let verified = ledger.verify_address(&path)?;

// Sign a transaction using intent-based signing
let signed_tx = ledger.sign_intent(&path, &address, intent, &transaction_data, objects)?;
```

## Transport Types

The crate supports two transport mechanisms:

- **Native HID**: Direct USB communication with physical Ledger devices
- **TCP**: Communication with Speculos simulator (default port 9999)

Set the `LEDGER_SIMULATOR` environment variable to automatically use TCP transport.

## Examples

The crate includes several examples in the `examples/` directory:

- `ledger_get_public_key.rs`: Retrieve public keys and addresses
- `ledger_sign_tx.rs`: Sign transactions with the Ledger device
- `ledger_open_app.rs`: Ensure the IOTA app is open and read the version

Run examples with:

```bash
cargo run --example ledger_get_public_key -- --path "m/44'/4218'/0'/0'/0'"
```
