# Amplitude Events Tracking List for IOTA EVM Bridge

This document outlines user action events to track with Amplitude for analytics and user behavior analysis in the IOTA EVM Bridge application.

## Table of Contents

1. [Wallet Actions](#wallet-actions)
2. [Bridge Configuration](#bridge-configuration)
3. [Bridge Transactions](#bridge-transactions)
4. [Form Interactions](#form-interactions)
5. [Error Events](#error-events)

---

## Wallet Actions

- **Event**: `user connected l1 wallet`
  - **Description**: User successfully connected their IOTA L1 wallet
  - **Properties**: `wallet_type`, `address`

- **Event**: `user connected l2 wallet`
  - **Description**: User successfully connected their L2 wallet (MetaMask/RainbowKit)
  - **Properties**: `wallet_type`, `address`, `chain_id`

- **Event**: `user requested faucet funds`
  - **Description**: User clicked to request test funds from faucet
  - **Properties**: `address`, `success`

---

## Bridge Configuration

- **Event**: `user toggled bridge direction`
  - **Description**: User switched bridge direction between L1→L2 and L2→L1
  - **Properties**: `from_layer`, `to_layer`

- **Event**: `user selected coin`
  - **Description**: User selected a different coin type for bridging
  - **Properties**: `coin_type`, `coin_symbol`, `bridge_direction`

- **Event**: `user clicked max amount`
  - **Description**: User clicked "Max" button to use full available balance
  - **Properties**: `coin_type`, `bridge_direction`, `max_amount`

---

## Bridge Transactions

- **Event**: `user sent from l1 to l2`
  - **Description**: User initiated a deposit transaction from IOTA L1 to IOTA EVM
  - **Properties**: `amount`, `coin_type`, `receiving_address`, `gas_estimate`

- **Event**: `user sent from l2 to l1`
  - **Description**: User initiated a withdraw transaction from IOTA EVM to IOTA L1
  - **Properties**: `amount`, `coin_type`, `receiving_address`, `gas_estimate`

- **Event**: `user cancelled transaction`
  - **Description**: User rejected/cancelled transaction in wallet
  - **Properties**: `transaction_type`, `amount`, `coin_type`

---

## Form Interactions

- **Event**: `user entered amount`
  - **Description**: User manually entered a bridge amount
  - **Properties**: `amount`, `coin_type`, `bridge_direction`

- **Event**: `user toggled address input`
  - **Description**: User switched between manual address entry and wallet auto-fill
  - **Properties**: `input_mode` (manual, auto), `bridge_direction`

- **Event**: `user entered receiving address`
  - **Description**: User manually entered a receiving address
  - **Properties**: `address_format` (iota, evm), `bridge_direction`

---

## Error Events

- **Event**: `user encountered insufficient balance`
  - **Description**: User attempted to bridge more than available balance
  - **Properties**: `requested_amount`, `available_balance`, `coin_type`

- **Event**: `user entered invalid address`
  - **Description**: User entered an invalid receiving address format
  - **Properties**: `address_format` (iota, evm), `bridge_direction`

- **Event**: `user experienced transaction failure`
  - **Description**: User's transaction failed after submission
  - **Properties**: `transaction_type`, `error_type`, `amount`, `coin_type`

---

## Event Properties Schema

### Common Properties (included in all events)
- `timestamp`: ISO 8601 timestamp
- `session_id`: Unique session identifier
- `user_id`: Anonymous user identifier
- `app_version`: Application version
- `environment`: deployment environment (development, testnet, mainnet)
- `user_agent`: Browser user agent string
- `viewport_size`: Browser viewport dimensions

### Bridge-Specific Properties
- `bridge_direction`: "l1_to_l2" | "l2_to_l1"
- `coin_type`: Full coin type identifier
- `coin_symbol`: Human-readable coin symbol (IOTA, etc.)
- `amount`: Numeric amount in human-readable format
- `address`: Wallet address (anonymized if needed)
- `wallet_type`: Type of connected wallet (iota_wallet, metamask, rainbow, etc.)
- `chain_id`: EVM chain identifier
- `gas_estimate`: Estimated gas cost
- `input_mode`: "manual" | "auto"
- `address_format`: "iota" | "evm"
- `transaction_type`: "deposit" | "withdraw"
- `error_type`: Categorized error type
- `from_layer`: "l1" | "l2"
- `to_layer`: "l1" | "l2"
- `success`: boolean indicating operation success
