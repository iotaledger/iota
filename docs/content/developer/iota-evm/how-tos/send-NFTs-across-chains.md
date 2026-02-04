---
image: /img/logo/WASP_logo_dark.png
tags:
  - evm
  - solidity
  - explanation
  - how-to
teams:
  - iotaledger/l2-smart-contract
---

# Send NFTs Across Chains

## Introduction

[LayerZero ONFT V2](https://docs.layerzero.network/v2/developers/evm/onft/quickstart) enables cross-chain transfers of existing ERC721 tokens. For
testing purposes, the IOTA EVM Testnet is chosen as the source chain, while the BNB Testnet is chosen as the destination
chain.

:::info ONFT Package

To use ONFT in your project, install the official LayerZero ONFT package:

```bash
npm install @layerzerolabs/onft-evm
```

See [@layerzerolabs/onft-evm](https://www.npmjs.com/package/@layerzerolabs/onft-evm) for package details.

:::

## Why Would a User Need to Send ERC721 Tokens Across Chains?

By facilitating the movement of ERC721 tokens across chains, users gain flexibility and can optimize their NFT usage
according to their specific needs, preferences, and circumstances.

### Enable the Existing ERC721 Tokens for Cross-Chain Sending

To enable the existing ERC721 tokens for cross-chain sending, you will need the `ONFT721Adapter` contract on the source
chain, and the `ONFT721` contract on the destination chain.

The origin NFT token will be locked in the `ONFT721Adapter` contract so that the ONFT-wrapped tokens will be minted on the
destination chain. If the NFT token already exists on the destination chain (i.e., when the ONFT-wrapped token on
the destination chain is sent back to the source chain), no new token minting will happen. Instead, the NFT tokens will be
transferred from the `ONFT721` contract to the user's wallet address.

### Enable Cross-Chain Sending for New ERC721 NFTs

If you are launching a new ERC721 token, you can use the `ONFT721` standard to enable cross-chain sending without the need of
`ONFT721Adapter`. The NFT will be burned on the source chain and minted on the destination chain.

:::info Contract Documentation

- [ONFT721Adapter](https://docs.layerzero.network/v2/developers/evm/onft/quickstart#onft721adapter-implementation)
- [ONFT721](https://docs.layerzero.network/v2/developers/evm/onft/quickstart#onft721-implementation)

:::

## Scripts

### Deploy the ONFT721Adapter and ONFT721 Contracts

#### For ERC721

- MyONFT721Adapter.sol (on source chain where original ERC721 exists):
  - CTOR:
    - `_token`: deployed contract address of the existing ERC721 tokens on the source chain.
    - `_lzEndpoint`: LayerZero Endpoint V2 on the source chain.
    - `_delegate`: owner/governance address for OApp configurations.

- MyONFT721.sol (on destination chain):
  - CTOR:
    - `_name`: name of the ONFT-wrapped tokens on the destination chain
    - `_symbol`: symbol of the ONFT-wrapped tokens on the destination chain
    - `_lzEndpoint`: LayerZero Endpoint V2 on the destination chain
    - `_delegate`: owner/governance address for OApp configurations.

### Set the Remote Peer

For **existing ERC721 tokens**, the `ONFT721Adapter` and `ONFT721` contract instances must be paired.

For the **new ERC721 tokens** that want to leverage the `ONFT721` standard, the `ONFT721` contract instance on the source chain
needs to be paired with another `ONFT721` contract instance on the destination chain.

You can set this using the [`setPeer`](https://docs.layerzero.network/v2/developers/evm/onft/quickstart#3-configure-peer-relationships) method.

### Set Enforced Options

Both the `ONFT721Adapter` and the `ONFT721` contract instances need to be configured with enforced options for minimum gas on the destination chain.

You can set this using the [`setEnforcedOptions`](https://docs.layerzero.network/v2/developers/evm/onft/quickstart#4-configure-message-execution-options) method.

:::info

The `enforcedOptions` define minimum gas requirements that every `send` must abide by (e.g., minimum gas for `lzReceive`).

:::

### Configure DVN (Recommended)

Configure the Decentralized Verifier Network (DVN) settings for enhanced security. The `requiredDVNs` should be set on both chains.

See [DVN Configuration](https://docs.layerzero.network/v2/developers/evm/configuration/dvn-executor-config) for details.

## How To Send Tokens From a Source Chain to a Destination Chain (and Vice-Versa)

### Required Contracts

#### From the Source Chain to the Destination Chain

For the existing ERC721 tokens, you will need the `ONFT721Adapter` contract on the source chain and the `ONFT721` contract on
the destination chain. The procedure is as follows:

1. The sender approves his ERC721 tokens for the `ONFT721Adapter` contract.
2. The sender calls the function [`quoteSend()`](https://docs.layerzero.network/v2/developers/evm/onft/quickstart#estimating-gas-fees) of the ONFT721Adapter contract to estimate cross-chain fee to be paid in
   native on the source chain.
3. The sender calls the function [`send()`](https://docs.layerzero.network/v2/developers/evm/onft/quickstart#sending-nfts-across-chains) of the ONFT721Adapter contract to transfer tokens on source chain to destination
   chain.
4. (Optional) Wait for the transaction finalization on the destination chain by using the
   [@layerzerolabs/scan-client](https://www.npmjs.com/package/@layerzerolabs/scan-client#example-usage) library.

#### From the Destination Chain Back to the Source Chain

To send back the ONFT-wrapped tokens on the destination chain to the source chain, the procedure is similar as the
approve step is also required, but the operations will happen on the `ONFT721` contract.

#### References and Tools

##### `SendParam` and Options

- You can use the [LayerZero Options Builder](https://docs.layerzero.network/v2/developers/evm/gas-settings/options) to configure `extraOptions` in the `SendParam` struct.
- For gas drop on the destination, use `addExecutorNativeDropOption()` in the options builder.

##### LayerZero

- [LayerZero Endpoint V2](https://docs.layerzero.network/v2/deployments/deployed-contracts)
- [LayerZero explorer](https://layerzeroscan.com/)

### Create a New ONFT Project

The easiest way to get started is using the LayerZero CLI:

```bash
npx create-lz-oapp@latest
```

When prompted, choose `ONFT721` as the starting point. This will create both `ONFT721` and `ONFT721Adapter` contracts for your project.

Alternatively, add the ONFT package to an existing project:

```bash
npm install @layerzerolabs/onft-evm
```

### Compile the Contracts

After setting up your project, compile the contracts:

```bash
npx hardhat compile
```

### Set Your Configuration

Copy the `.env.example` file to `.env` and configure your private key and RPC endpoints:

```bash
cp .env.example .env
```

Update `hardhat.config.ts` to include IOTA EVM network configuration:

```typescript
iotaEvmTestnet: {
  eid: EndpointId.IOTA_V2_TESTNET,
  url: 'https://json-rpc.evm.testnet.iotaledger.net',
  accounts: [process.env.PRIVATE_KEY],
},
iotaEvmMainnet: {
  eid: EndpointId.IOTA_V2_MAINNET,
  url: 'https://json-rpc.evm.iotaledger.net',
  accounts: [process.env.PRIVATE_KEY],
},
```

### Deploy the Contracts

Use the LayerZero CLI to deploy your contracts:

```bash
npx hardhat lz:deploy
```

You'll be prompted to select which chains to deploy to. For this example:

- Deploy `ONFT721Adapter` on IOTA EVM (source chain where original ERC721 exists)
- Deploy `ONFT721` on BNB Testnet (destination chain)

### Wire the Contracts (Set Peers)

After deployment, connect your contracts across chains:

```bash
npx hardhat lz:oapp:wire --oapp-config layerzero.config.ts
```

This command automatically calls `setPeer` on each contract to establish trust between ONFT contracts on different chains.

### Verify Setup

Verify that the peers were configured correctly:

```bash
npx hardhat lz:oapp:peers:get --oapp-config layerzero.config.ts
```

### Configure Enforced Options (Optional)

If you need custom gas settings, configure `enforcedOptions` in your `layerzero.config.ts`:

```typescript
import { Options } from '@layerzerolabs/lz-v2-utilities'

// Enforce minimum 100,000 gas for lzReceive on destination
const enforcedOptions = Options.newOptions()
  .addExecutorLzReceiveOption(100_000, 0)
  .toHex()
```

Then run the wire command to apply:

```bash
npx hardhat lz:oapp:wire --oapp-config layerzero.config.ts
```

### Send NFTs Across Chains

#### From IOTA EVM to Destination Chain

Use the Hardhat task to send an NFT:

```bash
npx hardhat send-nft \
  --adapter <ONFT721Adapter_ADDRESS> \
  --dst-endpoint-id 30102 \
  --recipient <RECIPIENT_ADDRESS> \
  --token-id <TOKEN_ID> \
  --network iotaEvmTestnet
```

The task will:

1. Approve the NFT for the adapter contract
2. Call `quoteSend()` to estimate fees
3. Call `send()` to transfer the NFT cross-chain

#### From Destination Chain Back to IOTA EVM

```bash
npx hardhat send-nft \
  --adapter <ONFT721_ADDRESS> \
  --dst-endpoint-id 30284 \
  --recipient <RECIPIENT_ADDRESS> \
  --token-id <TOKEN_ID> \
  --network bnbTestnet
```
