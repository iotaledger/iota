# IOTA-Names SDK

[![npm version](https://img.shields.io/npm/v/@iota/iota-names-sdk.svg)](https://www.npmjs.com/package/@iota/iota-names-sdk)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](../LICENSE)

A TypeScript SDK for interacting with the IOTA Name Service. This SDK provides a convenient wrapper for querying detailed information and building transactions for IOTA-Names.

> **Note:** You do not need to use this SDK for basic name resolution (name → address, address → name). That functionality is already covered by JSONRPC & GraphQL APIs.

## Installation

```bash
npm install @iota/iota-names-sdk
```

Or with yarn:

```bash
yarn add @iota/iota-names-sdk
```

Or with pnpm:

```bash
pnpm add @iota/iota-names-sdk
```

## Requirements

- Node.js 22+

## Basic Usage

### Initialize the Client

```typescript
import { IotaNamesClient } from '@iota/iota-names-sdk';
import { getNetwork, Network } from '@iota/iota-sdk/client';
import { IotaGraphQLClient } from '@iota/iota-sdk/graphql';

// Initialize the SDK client
const network = getNetwork(Network.Mainnet); // or Network.Testnet, Network.Devnet
const iotaNamesClient = new IotaNamesClient({
    graphQlClient: new IotaGraphQLClient({
        url: network.graphql!,
    }),
    network: network.id,
});
```

### Register a Name

```typescript
import { IotaNamesTransaction } from '@iota/iota-names-sdk';
import { Transaction } from '@iota/iota-sdk/transactions';

// Create a transaction to register a name
const tx = new Transaction();
const iotaNamesTx = new IotaNamesTransaction(iotaNamesClient, tx);
const [coin] = iotaNamesTx.transaction.splitCoins(tx.gas, [10_000_000]);

// Register the name
const nft = await iotaNamesTx.register({
    name: 'mycoolname.iota',
    coin,
});

// Transfer the NFT to your address
iotaNamesTx.transaction.transferObjects([nft], address);

//Build transaction
const transaction = await iotaNamesTx.transaction.build({
    client,
});
```

### Query Name Information

```typescript
// Get name record
const nameRecord = await iotaNamesClient.getNameRecord('example.iota');
console.log(nameRecord);

// Get price lists
const priceList = await iotaNamesClient.getPriceList();
const renewalPriceList = await iotaNamesClient.getRenewalPriceList();
```

## Features

- Name registration and management
- Price querying
- Setting and updating name metadata
- Subname management
- Support for coupons and discounts
- Handling name expiration and renewals

## Examples

The SDK includes several examples that demonstrate common use cases:

- Basic name registration
- Setting name record data
- Using coupons
- Command center operations
- Working with subnames

For detailed examples, check the `examples` directory in the repository.

## API Reference

For complete API documentation, visit the [IOTA-Names Documentation](https://docs.iotanames.com).

## License

This project is licensed under the Apache License 2.0. See the `LICENSE` file for details.

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

For more information about IOTA-Names, visit the [official documentation](https://docs.iotanames.com).
