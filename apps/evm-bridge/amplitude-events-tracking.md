# Amplitude Events Tracking List for IOTA EVM Bridge

This document outlines the comprehensive list of events that should be tracked with Amplitude for analytics and user behavior analysis in the IOTA EVM Bridge application.

## Table of Contents

1. [Wallet Connection](#wallet-connection)
2. [Bridge Operations](#bridge-operations)
3. [Transaction Management](#transaction-management)
4. [User Interface Interactions](#user-interface-interactions)
5. [Error & Validation Events](#error--validation-events)
6. [Preferences](#preferences)

---

## Wallet Connection

### L1 Wallet Connection

- **Event**: `l1_wallet_connect_initiated`
  - **Description**: User clicks "Connect L1 Wallet" button
  - **Properties**: `button_location` (header_desktop, header_mobile, form)

- **Event**: `l1_wallet_connected`
  - **Description**: L1 wallet successfully connected
  - **Properties**: `wallet_type`, `address`, `connection_method`

- **Event**: `l1_wallet_connection_failed`
  - **Description**: L1 wallet connection failed
  - **Properties**: `error_type`, `error_message`, `wallet_type`

### L2 Wallet Connection

- **Event**: `l2_wallet_connect_initiated`
  - **Description**: User clicks "Connect L2 Wallet" button
  - **Properties**: `button_location` (header_desktop, header_mobile, form)

- **Event**: `l2_wallet_connected`
  - **Description**: L2 wallet successfully connected (MetaMask/RainbowKit)
  - **Properties**: `wallet_type`, `address`, `chain_id`, `connection_method`

- **Event**: `l2_wallet_connection_failed`
  - **Description**: L2 wallet connection failed
  - **Properties**: `error_type`, `error_message`, `wallet_type`

### Wallet Disconnection

- **Event**: `l1_wallet_disconnected`
  - **Description**: L1 wallet disconnected
  - **Properties**: `disconnection_method` (user_action, session_timeout)

- **Event**: `l2_wallet_disconnected`
  - **Description**: L2 wallet disconnected
  - **Properties**: `disconnection_method` (user_action, session_timeout)

---

## Bridge Operations

### Bridge Direction

- **Event**: `bridge_direction_toggled`
  - **Description**: User toggles bridge direction (L1→L2 or L2→L1)
  - **Properties**: `from_direction` (layer1, layer2), `to_direction` (layer1, layer2)

### Coin Selection

- **Event**: `coin_selected`
  - **Description**: User selects a coin type for bridging
  - **Properties**: `coin_type`, `coin_symbol`, `bridge_direction`, `available_balance`

### Amount Input

- **Event**: `max_amount_clicked`
  - **Description**: User clicks "Max" button to use full balance
  - **Properties**: `available_balance`, `coin_type`, `bridge_direction`

### Address Management

- **Event**: `receiving_address_toggled`
  - **Description**: User toggles between connected wallet and manual address input
  - **Properties**: `input_method` (connected_wallet, manual_input), `bridge_direction`

- **Event**: `manual_address_entered`
  - **Description**: User manually enters receiving address
  - **Properties**: `address_type` (iota, evm), `bridge_direction`

---

## Transaction Management

### Transaction Initiation

- **Event**: `bridge_transaction_initiated`
  - **Description**: User clicks "Bridge Assets" button
  - **Properties**: `amount`, `coin_type`, `bridge_direction`, `receiving_address`, `gas_estimate_iota`, `gas_estimate_evm`

### L1 Deposit Transactions

- **Event**: `l1_deposit_transaction_submitted`
  - **Description**: L1 deposit transaction submitted to network
  - **Properties**: `amount`, `coin_type`, `receiving_address`, `transaction_hash`, `gas_fee`

- **Event**: `l1_deposit_transaction_confirmed`
  - **Description**: L1 deposit transaction confirmed
  - **Properties**: `amount`, `coin_type`, `transaction_hash`, `confirmation_time`, `gas_used`

- **Event**: `l1_deposit_transaction_failed`
  - **Description**: L1 deposit transaction failed
  - **Properties**: `amount`, `coin_type`, `error_type`, `error_message`, `transaction_hash`

### L2 Withdraw Transactions

- **Event**: `l2_withdraw_transaction_submitted`
  - **Description**: L2 withdraw transaction submitted to EVM network
  - **Properties**: `amount`, `coin_type`, `receiving_address`, `transaction_hash`, `gas_fee`

- **Event**: `l2_withdraw_transaction_confirmed`
  - **Description**: L2 withdraw transaction confirmed
  - **Properties**: `amount`, `coin_type`, `transaction_hash`, `confirmation_time`, `gas_used`

- **Event**: `l2_withdraw_transaction_failed`
  - **Description**: L2 withdraw transaction failed
  - **Properties**: `amount`, `coin_type`, `error_type`, `error_message`, `transaction_hash`

### Transaction Cancellation

- **Event**: `transaction_cancelled_by_user`
  - **Description**: User cancels transaction in wallet
  - **Properties**: `transaction_type` (l1_deposit, l2_withdraw), `amount`, `coin_type`, `cancellation_stage`

---

## User Interface Interactions

### Form Validation

- **Event**: `gas_estimation_loaded`
  - **Description**: Gas estimation successfully loaded
  - **Properties**: `iota_gas_estimate`, `evm_gas_estimate`, `bridge_direction`, `amount`

- **Event**: `gas_estimation_failed`
  - **Description**: Gas estimation failed to load
  - **Properties**: `error_message`, `bridge_direction`, `amount`

---

## Error & Validation Events

### Balance Errors

- **Event**: `insufficient_balance_error`
  - **Description**: User attempts to bridge more than available balance
  - **Properties**: `requested_amount`, `available_balance`, `coin_type`, `bridge_direction`

### Address Validation Errors

- **Event**: `invalid_address_error`
  - **Description**: User enters invalid receiving address
  - **Properties**: `entered_address`, `expected_format` (iota, evm), `bridge_direction`

### Network Errors

- **Event**: `network_connection_error`
  - **Description**: Network connection error during operation
  - **Properties**: `operation_type`, `network` (iota, evm), `error_message`

### Transaction Errors

- **Event**: `transaction_build_error`
  - **Description**: Error building transaction before submission
  - **Properties**: `transaction_type`, `error_message`, `amount`, `coin_type`

---

## Preferences

### Transaction Monitoring

- **Event**: `transaction_receipt_polling_started`
  - **Description**: Started polling for transaction receipt
  - **Properties**: `transaction_hash`, `transaction_type`, `polling_interval`

- **Event**: `transaction_receipt_received`
  - **Description**: Transaction receipt successfully received
  - **Properties**: `transaction_hash`, `polling_attempts`, `time_to_receipt`

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
- `bridge_direction`: "layer1_to_layer2" | "layer2_to_layer1"
- `coin_type`: Full coin type identifier
- `coin_symbol`: Human-readable coin symbol
- `amount`: Numeric amount (in smallest unit)
- `formatted_amount`: Human-readable amount with decimals
- `gas_estimate_iota`: IOTA gas estimation
- `gas_estimate_evm`: EVM gas estimation
- `transaction_hash`: Blockchain transaction hash
- `wallet_type`: Type of connected wallet
- `network`: "iota" | "evm"
- `error_type`: Categorized error type
- `error_message`: Detailed error message
