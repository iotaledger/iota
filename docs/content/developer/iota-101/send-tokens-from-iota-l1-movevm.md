---
title: Bridge Tokens from IOTA L1 MoveVM
sidebar_label: Bridge Tokens from IOTA L1 MoveVM
description: Learn how to bridge tokens from IOTA L1 MoveVM to other chains using LayerZero OFT V2.
---

# Send Tokens From IOTA L1 MoveVM to Other Chains

## Introduction

[LayerZero OFT V2](https://docs.layerzero.network/v2) enables cross-chain transfers of fungible tokens from IOTA L1 (MoveVM) to other blockchain networks. This guide focuses on sending tokens from IOTA L1 to two destination chain types:

1. **IOTA L1 → IOTA EVM**: Bridge tokens between IOTA's native L1 and its EVM-compatible layer
2. **IOTA L1 → Sui MoveVM**: Bridge tokens between two Move-based blockchains

:::info Community Libs

There are two utility repositories for LayerZero OFT V2:

- **MoveVM side**: [layerzero-move-oft-v2-utils](https://github.com/iota-community/layerzero-move-oft-v2-utils) - For deploying and managing OFT modules on IOTA L1 MoveVM
- **EVM side**: [layerzero-oft-v2-utils](https://github.com/iota-community/layerzero-oft-v2-utils/tree/movevm) - For deploying and managing OFT/OFTAdapter contracts on EVM chains

:::

### Why Send Tokens From IOTA L1?

IOTA L1 MoveVM provides a highly secure and scalable foundation for token operations. By bridging tokens from IOTA L1, you can:

- Access liquidity on EVM-compatible chains.
- Interact with other Move-based ecosystems.
- Leverage unique features of each destination chain.

### Two Use Cases

#### Existing Tokens (OFT Adapter)

For existing fungible tokens on IOTA L1, you need:
- **OFT Adapter** on IOTA L1: Locks/unlocks existing tokens
- **OFT** on destination chain: Mints/burns equivalent tokens

#### New Cross-chain Tokens (Native OFT)

For brand-new tokens designed for cross-chain use:
- **OFT** on IOTA L1: Mints/burns tokens natively
- **OFT** on destination chain: Mints/burns equivalent tokens

## Cross-Chain Transfer Overview

The cross-chain token transfer involves four main steps:

1. **Configuration** - Set up environment variables and config files
2. **Deployment** - Deploy OFT modules on both chains
3. **Setup** - Configure peers, register OFT, and set DVN settings
4. **Send** - Execute the cross-chain transfer

:::tip Further Information

- [LayerZero OFT Documentation](https://docs.layerzero.network/v2/developers/evm/oft/quickstart)
- [LayerZero IOTA L1 Documentation](https://docs.layerzero.network/v2/developers/iota/overview)

:::

## Pathway 1: IOTA L1 ↔ IOTA EVM

This pathway enables token transfers between IOTA L1 (MoveVM) and IOTA EVM using LayerZero OFT V2.

### Architecture

```
IOTA L1 (MoveVM)                      IOTA EVM
┌──────────────────────┐              ┌──────────────────────┐
│  OFT Module          │◀────────────▶│  OFT Contract        │
│  (Burn/Mint)         │  LayerZero   │  (Burn/Mint)         │
└──────────────────────┘              └──────────────────────┘
     Endpoint ID: 30423                    Endpoint ID: 30284
```

### Step 1: Clone and Install Dependencies

```bash
git clone https://github.com/iota-community/layerzero-move-oft-v2-utils.git
cd layerzero-move-oft-v2-utils
yarn install
```

### Step 2: Configure Environment

Copy the configuration template and set up your environment:

```bash
cp .env.example .env
cp config_IotaEVM_IOTAL1.ts config.ts
```

Edit `.env` with your settings:

```bash
# Network: testnet or mainnet
NETWORK='mainnet'

# MNEMONIC of the account that deployed the OFT Move module
MNEMONIC='your twelve word mnemonic phrase here'

# Recipient address on the destination chain (IOTA EVM)
REMOTE_RECIPIENT_ADDRESS='0xYourEVMAddress'

# Amount to send (without decimals, e.g., 5 for 5 tokens)
TOKEN_AMOUNT_WITHOUT_DECIMALS=5
```

Edit `config.ts` with your deployed contract information:

```typescript
export default {
  mainnet: {
    sharedDecimals: 6,
    oft: {
      oftPackageId: '0xYourOFTPackageId',
      oappObjectId: '0xYourOAppObjectId',
      upgradeCap: '0xYourUpgradeCap',
      oftInitTicketId: '0xYourOFTInitTicketId',
    },
    coin: {
      coinPackage: '0xYourCoinPackage',
      coinType: '0xYourCoinPackage::your_coin::YOUR_COIN',
      coinDecimals: 9,
      upgradeCap: '0xYourCoinUpgradeCap',
      treasuryCapId: '0xYourTreasuryCapId',
      metadataId: '0xYourMetadataId',
    },
    oftObjectId: '0xYourOFTObjectId',  // Set after init
    oftComposerManagerId: '0xfe5be5a2d5b11e635e3e4557bb125fb24a3dd09111eded06fd6058b2aee1d054',
    remoteChain: {
      EID: 30284,  // IOTA EVM Mainnet
      peerAddress: '0xYourEVMOFTContractAddress',
    },
    setConfig: {
      DVNs: ['0xa560697328ccb5dc3f3f8e8a2c41e282827060da7a29971d933e9aa405c2ba7f'],  // LayerZero Labs DVN
      confirmations: 25,
    },
  },
};
```

### Step 3: Deploy OFT Module on IOTA L1

Deploy the OFT Move module from the official LayerZero repository:

```bash
git clone https://github.com/LayerZero-Labs/LayerZero-v2.git
cd LayerZero-v2/packages/layerzero-v2/iota/contracts/oapps/oft/oft
iota client publish --gas-budget 500000000
```

After deployment, note down the following from the output:
- **OFT Package ID**
- **OFTInitTicket Object ID**
- **OApp Object ID**
- **UpgradeCap Object ID**

Update your `config.ts` with these values.

### Step 4: Initialize OFT

Choose the appropriate initialization based on your use case:

#### For Existing Tokens (OFT Adapter):

```bash
yarn init-oft-adapter
```

#### For New Tokens (Native OFT):

```bash
yarn init-oft
```

After initialization, get the `oftObjectId` from the transaction output and update your `config.ts`.

### Step 5: Register OFT

Register the OFT with the LayerZero endpoint:

```bash
yarn register-oft
```

Expected output:

```bash
oft.registerOAppMoveCall
oftComposerManagerId: 0xfe5be5a2d5b11e635e3e4557bb125fb24a3dd09111eded06fd6058b2aee1d054
senderAddr: 0xYourAddress
inspectTx result: { status: 'success' }
executeTx - Tx hash: <TX_HASH>
```

### Step 6: Set Peer

Link the IOTA L1 OFT with the IOTA EVM OFT contract:

```bash
yarn set-peer-oft
```

:::warning Required: EVM Side Setup

You must also deploy and configure the OFT contract on IOTA EVM. See the [EVM utilities repository](https://github.com/iota-community/layerzero-oft-v2-utils/tree/movevm) for instructions.

The peer relationship must be established on **both sides**.

:::

### Step 7: Set DVN Configuration

Configure the DVN (Decentralized Verifier Network) settings:

```bash
yarn set-config
```

:::danger DVN Consistency

Each chain uses its own DVN address format. Ensure you use the correct DVN for each chain:

**IOTA L1 Mainnet DVN (LayerZero Labs):**
```
0xa560697328ccb5dc3f3f8e8a2c41e282827060da7a29971d933e9aa405c2ba7f
```

**IOTA EVM Mainnet DVN (LayerZero Labs):**
```
0xdd7b5e1db4aafd5c8ec3b764efb8ed265aa5445b
```

Mismatched DVN configurations will cause transactions to get stuck in "inflight" status.

:::

### Step 8: Send Tokens from IOTA L1 to IOTA EVM

Execute the cross-chain transfer:

```bash
yarn send-oft
```

Expected output:

```bash
oft.quoteSend and oft.sendMoveCall
oftQuote: {
  limit: { minAmountLd: 0n, maxAmountLd: 18446744073709551615n },
  feeDetails: [],
  receipt: { amountSentLd: 5000000000n, amountReceivedLd: 5000000000n }
}
No OFT fees
messagingFee: { nativeFee: 209016263n, zroFee: 0n }
senderAddr: 0xYourAddress
inspectTx result: { status: 'success' }
executeTx - Tx hash: CRY6W9sdo6AUfefhUdEmCZEfHDxbbPbex7ab8dM88fck
```

Track your transaction on [LayerZero Scan](https://layerzeroscan.com/).

---

## Pathway 2: IOTA L1 ↔ Sui MoveVM

This pathway enables token transfers between IOTA L1 and Sui, both Move-based blockchains.

### Architecture

```
IOTA L1 (MoveVM)                      Sui (MoveVM)
┌──────────────────────┐              ┌──────────────────────┐
│  OFT Module          │◀────────────▶│  OFT Module          │
│  (Burn/Mint or       │  LayerZero   │  (Burn/Mint or       │
│   Lock/Unlock)       │              │   Lock/Unlock)       │
└──────────────────────┘              └──────────────────────┘
     Endpoint ID: 30423                    Endpoint ID: 30378
```

### Step 1: Install Dependencies

Same as Pathway 1:

```bash
git clone https://github.com/iota-community/layerzero-move-oft-v2-utils.git
cd layerzero-move-oft-v2-utils
yarn install
```

### Step 2: Configure for Sui Pathway

Copy and configure for the Sui pathway:

```bash
cp .env.example .env
```

For mainnet, create a `config.ts` based on the Sui mainnet configuration:

```typescript
export default {
  mainnet: {
    sharedDecimals: 6,
    oft: {
      oftPackageId: '0xYourOFTPackageId',
      oappObjectId: '0xYourOAppObjectId',
      upgradeCap: '0xYourUpgradeCap',
      oftInitTicketId: '0xYourOFTInitTicketId',
    },
    coin: {
      coinPackage: '0xYourCoinPackage',
      coinType: '0xYourCoinPackage::your_coin::YOUR_COIN',
      coinDecimals: 9,
      upgradeCap: '0xYourCoinUpgradeCap',
      treasuryCapId: '0xYourTreasuryCapId',
      metadataId: '0xYourMetadataId',
    },
    oftObjectId: '0xYourOFTObjectId',
    oftComposerManagerId: '0xfe5be5a2d5b11e635e3e4557bb125fb24a3dd09111eded06fd6058b2aee1d054',
    remoteChain: {
      EID: 30378,  // Sui Mainnet
      peerAddress: '0xYourSuiOFTPackageId',
    },
    setConfig: {
      DVNs: ['0xa560697328ccb5dc3f3f8e8a2c41e282827060da7a29971d933e9aa405c2ba7f'],  // LayerZero Labs DVN
      confirmations: 1,
    },
  },
};
```

Edit `.env`:

```bash
NETWORK='mainnet'
MNEMONIC='your twelve word mnemonic phrase here'
REMOTE_RECIPIENT_ADDRESS='0xYourSuiAddress'
TOKEN_AMOUNT_WITHOUT_DECIMALS=5
```

### Step 3: Deploy OFT Modules

Deploy OFT modules on both IOTA L1 and Sui following their respective documentation:

- **IOTA L1**: See Step 3 from Pathway 1
- **Sui**: Follow the [LayerZero Sui OFT SDK documentation](https://docs.layerzero.network/v2/developers/sui/oft/sdk)

### Step 4: Initialize, Register, and Set Peer

Follow the same steps as Pathway 1:

```bash
# Initialize (choose one)
yarn init-oft           # For new tokens
yarn init-oft-adapter   # For existing tokens

# Register
yarn register-oft

# Set peer
yarn set-peer-oft

# Set DVN config
yarn set-config
```

### Step 5: Send Tokens from IOTA L1 to Sui

```bash
yarn send-oft
```

Expected output:

```bash
oft.quoteSend and oft.sendMoveCall
oftQuote: {
  limit: { minAmountLd: 0n, maxAmountLd: 18446744073709551615n },
  feeDetails: [],
  receipt: { amountSentLd: 200000000n, amountReceivedLd: 200000000n }
}
No OFT fees
messagingFee: { nativeFee: 3402819282n, zroFee: 0n }
senderAddr: 0xYourAddress
inspectTx result: { status: 'success' }
executeTx - Tx hash: 77364jpdNA63vuHEFX3VqS1GY4hYiqKYV3QLva4n3rkn
```

---

## Sending Tokens Back to IOTA L1

To send OFT-wrapped tokens back from destination chains to IOTA L1:

### From IOTA EVM

Use the EVM utilities repository:

```bash
# Clone and setup
git clone -b movevm https://github.com/iota-community/layerzero-oft-v2-utils.git
cd layerzero-oft-v2-utils
yarn install

# Configure and send
export isForOFTAdapter=true && npx hardhat run scripts/send_oft.ts --network iotaEvmMainnet
```

### From Sui

If you're on the Sui side and want to send tokens to IOTA L1, follow the [LayerZero Sui documentation](https://docs.layerzero.network/v2/developers/sui/oft/sdk).

---

## LayerZero Endpoint IDs

| Network | Endpoint ID | Chain Type |
|---------|-------------|------------|
| IOTA L1 Mainnet | 30423 | MoveVM |
| IOTA EVM Mainnet | 30284 | EVM |
| Sui Mainnet | 30378 | MoveVM |
| IOTA L1 Testnet | 40423 | MoveVM |
| Sui Testnet | 40378 | MoveVM |

## Official DVN Addresses

### IOTA L1 Mainnet

| DVN Provider | Address |
|--------------|---------|
| LayerZero Labs | `0xa560697328ccb5dc3f3f8e8a2c41e282827060da7a29971d933e9aa405c2ba7f` |

### IOTA EVM Mainnet

| DVN Provider | Address |
|--------------|---------|
| LayerZero Labs | `0xdd7b5e1db4aafd5c8ec3b764efb8ed265aa5445b` |

## Protocol Contract Addresses

### IOTA L1 Mainnet

| Contract | Address |
|----------|---------|
| EndpointV2 | `0xb8e0cd76cb8916c48c03320e43d46c3775edd6f17ce7fbfad6c751289dcb1735` |
| SendUln302 | `0x042e3bb837e5528e495124542495b9df5016acd011d89838ae529db5a814499e` |
| ReceiveUln302 | `0x042e3bb837e5528e495124542495b9df5016acd011d89838ae529db5a814499e` |
| LZ Executor | `0x29b691f9496eea...` |

### IOTA EVM Mainnet

| Contract | Address |
|----------|---------|
| EndpointV2 | `0x1a44076050125825900e736c501f859c50fE728c` |
| SendUln302 | `0xC39161c743D0307EB9BCc9FEF03eeb9Dc4802de7` |
| ReceiveUln302 | `0xe1844c5D63a9543023008D332Bd3d2e6f1FE1043` |
| LZ Executor | `0xc097ab8CD7b053326DFe9fB3E3a31a0CCe3B526f` |

## Troubleshooting

### Transaction Stuck "Inflight"

**Cause:** Mismatched DVN configuration between source and destination chains.

**Solution:** Verify that the `DVNs` array in your config matches on both chains. Note that IOTA L1 and IOTA EVM use **different** DVN addresses (see Official DVN Addresses above).

### Peer Not Set Error

**Cause:** Peer relationship not established on one or both chains.

**Solution:** Ensure you've run `set-peer-oft` on the IOTA L1 side AND set the peer on the destination chain (EVM or Sui).

### Insufficient Gas

**Cause:** Not enough IOTA for transaction fees.

**Solution:** Ensure your account has sufficient IOTA balance. The `messagingFee.nativeFee` shown in the quote represents the required LayerZero fee in nanoIOTA.

### OFT Init Ticket Already Used

**Cause:** The `oftInitTicketId` can only be used once for initialization.

**Solution:** If you need to reinitialize, you must deploy a new OFT package to get a fresh `oftInitTicketId`.

## Verified Pathway Results

The following pathways have been tested and verified:

### Mainnet

| Pathway | Direction | LayerZero Scan Link |
|---------|-----------|---------------------|
| IOTA EVM → IOTA L1 | Forward | [View](https://layerzeroscan.com/tx/0x4d456538ec81679d3a1eedd2b404e6f847511d7b75b14398052e3287b9b1dce5) |
| IOTA L1 → IOTA EVM | Return | [View](https://layerzeroscan.com/tx/CRY6W9sdo6AUfefhUdEmCZEfHDxbbPbex7ab8dM88fck) |
| Sui → IOTA L1 | Forward | [View](https://layerzeroscan.com/tx/6CdWkpFZbhPro4w8eWvhDtwd6x3g8qqMVc9UVVVYqF5L) |
| IOTA L1 → Sui | Return | [View](https://layerzeroscan.com/tx/77364jpdNA63vuHEFX3VqS1GY4hYiqKYV3QLva4n3rkn) |
| Arbitrum → IOTA L1 | Forward | [View](https://layerzeroscan.com/tx/0xb7abd5db36d8f6407c707dff2e8f08d4d11e4188a16b39f0233fa21e94388a5e) |
| IOTA L1 → Arbitrum | Return | [View](https://layerzeroscan.com/tx/9QhCbuF9i4M863nERyFCd1cHqVggZfgTEetTY5Tny9WW) |

## References

- [LayerZero OFT V2 Documentation](https://docs.layerzero.network/v2/developers/evm/oft/quickstart)
- [LayerZero IOTA L1 SDK](https://www.npmjs.com/package/@layerzerolabs/lz-iotal1-oft-sdk-v2)
- [LayerZero Sui Documentation](https://docs.layerzero.network/v2/developers/sui/oft/sdk)
- [LayerZero Deployed Contracts](https://docs.layerzero.network/v2/deployments/deployed-contracts)
- [LayerZero Scan (Transaction Tracker)](https://layerzeroscan.com/)
- [IOTA L1 MoveVM Utilities](https://github.com/iota-community/layerzero-move-oft-v2-utils)
- [IOTA EVM Utilities](https://github.com/iota-community/layerzero-oft-v2-utils/tree/movevm)
