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

# Send ERC20 Tokens Between IOTA EVM and IOTA L1

## Introduction

[LayerZero OFT V2](https://docs.layerzero.network/v2) enables cross-chain transfers of fungible tokens between EVM chains and IOTA L1 (MoveVM). This guide focuses on bridging ERC20 tokens between IOTA EVM and IOTA L1.

:::info Community Libs

There are two utility repositories for LayerZero OFT V2:

- **EVM side**: [layerzero-oft-v2-utils](https://github.com/iota-community/layerzero-oft-v2-utils/tree/movevm) - For deploying and managing OFT/OFTAdapter contracts on EVM chains.
- **MoveVM side**: [layerzero-move-oft-v2-utils](https://github.com/iota-community/layerzero-move-oft-v2-utils) - For deploying and managing OFT modules on IOTA L1.

:::

### Why Would a User Need to Send ERC20 Tokens Across Chains?

Sending ERC20 tokens across chains allows users to leverage different blockchain networks' strengths and unique features, optimize costs, and manage risks more effectively. This flexibility is crucial as the blockchain ecosystem continues to grow and diversify.

#### Send Existing ERC20 Tokens Across Chains

You need both the [OFT Adapter](https://docs.layerzero.network/v2/developers/evm/oft/adapter) contract on the EVM source chain and the OFT contract on IOTA L1 to enable existing ERC20 tokens for cross-chain sending. The OFT Adapter locks/unlocks existing tokens, while the OFT mints/burns equivalent tokens on the destination chain.

#### Create New Cross-chain Fungible Tokens

If you are launching a new token, you can use the [OFT standard](https://docs.layerzero.network/v2/developers/evm/oft/quickstart) on EVM and the corresponding OFT module on IOTA L1 to enable cross-chain sending without the OFT Adapter.

## Cross-Chain Transfer Procedure

The cross-chain token transfer involves four steps:

1. **Configuration** - Set up environment variables and config files
2. **Deployment** - Deploy contracts on both chains
3. **Setup** - Configure peers, enforced options, and DVN settings
4. **Send** - Execute the cross-chain transfer

:::tip Further Information

- [LayerZero OFT Documentation](https://docs.layerzero.network/v2/developers/evm/oft/quickstart)
- [LayerZero Gas Settings](https://docs.layerzero.network/v2/developers/evm/gas-settings/options#option-types)
- [IOTA L1 LayerZero Deployment](https://docs.layerzero.network/v2/deployments/chains/iota-l1)

:::

### Send Tokens From IOTA EVM to IOTA L1 (and Vice Versa)

For existing ERC20 tokens, you will need the OFT Adapter contract on IOTA EVM and the OFT module on IOTA L1. The procedure on the EVM side is as follows:

#### 1. Approve the Tokens

The sender must approve their ERC20 tokens for the OFT Adapter contract.

```typescript
const approveTx = await erc20TokenContract.approve(oftAdapterContractAddress, amountInWei);
```

#### 2. Estimate the Fee

The sender calls the function `quoteSend()` of the OFT Adapter contract to estimate the cross-chain fee to be paid in native tokens on the source chain.

```typescript
const sendParam = [
  lzEndpointIdOnDestChain, // e.g., 30423 for IOTA L1 mainnet
  receiverAddressInBytes32,
  amountInWei,
  amountInWei,
  options, // additional options
  "0x", // composed message for the send() operation
  "0x", // OFT command to be executed, unused in default OFT implementations
];

const [nativeFee] = await myOFTAdapterContract.quoteSend(sendParam as any, false);
```

#### 3. Send the Tokens

The sender calls the function `send()` of the OFT Adapter contract to transfer tokens from IOTA EVM to IOTA L1.

```typescript
const sendTx = await myOFTAdapterContract.send(
  sendParam as any,
  [nativeFee, 0] as any, // set 0 for lzTokenFee
  sender.address, // refund address
  {
    value: nativeFee,
  },
);
const sendTxReceipt = await sendTx.wait();
console.log("sendOFT - send tx on source chain:", sendTxReceipt?.hash);
```

#### 4. (Optional) Wait for Finalization

The sender can wait for transaction finalization on the destination chain using the library [@layerzerolabs/scan-client](https://www.npmjs.com/package/@layerzerolabs/scan-client).

```typescript
const deliveredMsg = await waitForMessageReceived(
  Number(lzEndpointIdOnDestChain),
  sendTxReceipt?.hash as string,
);
console.log("sendOFT - received tx on destination chain:", deliveredMsg?.dstTxHash);
```

### Send the OFT-wrapped Tokens Back

To send back the OFT-wrapped tokens from IOTA L1 to IOTA EVM, you need to use the [layerzero-move-oft-v2-utils](https://github.com/iota-community/layerzero-move-oft-v2-utils) repository. The procedure is similar to sending tokens, but operates on the MoveVM side:

#### 1. Estimate the Fee

The sender calls `quoteSend()` to estimate the cross-chain fee:

```typescript
const sendParam = {
  dstEid: remoteChain.EID, // e.g., 30284 for IOTA EVM mainnet
  to: addressToBytes32(recipientAddress),
  amountLd: BigInt(amountInLocalDecimals),
  minAmountLd: BigInt(minAmount), // slippage protection
  extraOptions: Options.newOptions().addExecutorLzReceiveOption(200_000, 0).toBytes(),
  composeMsg: new Uint8Array(0),
  oftCmd: new Uint8Array(0),
};

const messagingFee = await oft.quoteSend(senderAddr, sendParam, false);
```

#### 2. Send the Tokens

The sender calls `sendMoveCall()` to transfer tokens from IOTA L1 back to IOTA EVM:

```typescript
const coin = await oft.splitCoinMoveCall(tx, senderAddr, sendParam.amountLd);

await oft.sendMoveCall(
  tx,
  senderAddr,
  sendParam,
  coin,
  messagingFee.nativeFee,
  messagingFee.zroFee,
  senderAddr, // refund address
);
```

For complete instructions, see the [MoveVM OFT Send Guide](https://github.com/iota-community/layerzero-move-oft-v2-utils/blob/main/README_OFT_send.md).

## EVM Utilities for LayerZero OFT V2

The [layerzero-oft-v2-utils](https://github.com/iota-community/layerzero-oft-v2-utils/tree/movevm) repository contains scripts for deploying and managing OFT contracts on EVM chains. For IOTA L1 (MoveVM) deployment and setup, see [layerzero-move-oft-v2-utils](https://github.com/iota-community/layerzero-move-oft-v2-utils).

### Install the Library

After you have cloned the [EVM utilities repository](https://github.com/iota-community/layerzero-oft-v2-utils/tree/movevm), run the following command to install:

```bash
yarn
```

### Compile the Contracts

Copy [`contracts-standard`](https://github.com/iota-community/layerzero-oft-v2-utils/tree/movevm/contracts-standard) (or [`contracts-mock`](https://github.com/iota-community/layerzero-oft-v2-utils/tree/movevm/contracts-mock) for testing) to `contracts`, then compile:

```bash
yarn compile
```

### Set Your Configuration

Copy the template `.env.example` to `.env` and configure accordingly. See [OFT Configuration](https://github.com/iota-community/layerzero-oft-v2-utils/blob/movevm/README_OFT_configuration.md) for details on config parameters.

### Deploy the Contracts

#### Deploy the OFT Adapter Contract on IOTA EVM

The OFT Adapter locks/unlocks existing ERC20 tokens on the source chain. When tokens are transferred, they get locked in the OFT Adapter and corresponding tokens are minted on IOTA L1 through the paired OFT module.

```bash
npx hardhat run scripts/deploy_oft_adapter.ts --network iotaEvmMainnet
```

Expected log output:

```bash
Deployed OFTAdapter contract address: <YOUR_DEPLOYED_CONTRACT_ADDRESS>
```

#### Deploy OFT on IOTA L1 (MoveVM)

For deploying OFT on IOTA L1, refer to the [layerzero-move-oft-v2-utils deployment guide](https://github.com/iota-community/layerzero-move-oft-v2-utils/blob/main/README_OFT_deployment.md).

### Setup the Contracts

After deploying contracts on both chains, you must configure them to communicate with each other.

#### Set Enforced Options

Configure gas settings for cross-chain messages:

```bash
export isForOFTAdapter=true && npx hardhat run scripts/set_enforced_options.ts --network iotaEvmMainnet
```

#### Set Remote Peer

Link the OFT Adapter on EVM with the OFT module on IOTA L1:

```bash
npx hardhat run scripts/set_peer_oft_adapter.ts --network iotaEvmMainnet
```

:::warning Required Configuration

You must also set the peer on the IOTA L1 side. See [MoveVM setup guide](https://github.com/iota-community/layerzero-move-oft-v2-utils/blob/main/README_OFT_setup.md).

:::

#### Set Config for DVN (Mandatory)

Configure the DVN (Decentralized Verifier Network) settings. The `requiredDVNs` must be the same on both chains, otherwise transactions will get stuck in `inflight` status.

```bash
export isForOFTAdapter=true && npx hardhat run scripts/set_config.ts --network iotaEvmMainnet
```

For detailed setup instructions, see [OFT Setup Guide](https://github.com/iota-community/layerzero-oft-v2-utils/blob/movevm/README_OFT_setup.md).

### Send Tokens from IOTA EVM to IOTA L1

Use the following command to send tokens from IOTA EVM to IOTA L1:

```bash
export isForOFTAdapter=true && npx hardhat run scripts/send_oft.ts --network iotaEvmMainnet
```

Expected log output:

```bash
sendOFT - oftAdapterContractAddress:0x02AE4418F0FbcbE383b4eD103cf6B88B24542f4C, lzEndpointIdOnRemoteChain:30423, executorLzReceiveOptionMaxGas:200000, receivingAccountAddress:0xd390...bd639, sender: 0x6B42...1e26b, amount:5
sendOFT - estimated nativeFee: 1.613365541632283385
sendOFT - send tx on source chain: 0x4d456538ec81679d3a1eedd2b404e6f847511d7b75b14398052e3287b9b1dce5
Wait for cross-chain tx finalization by LayerZero ...
sendOFT - received tx on destination chain: 8LDDk9xa6W4eGNrB7dSzWxH7hgFNwsWoDyzgPuF7vveg
```

### Send Tokens Back from IOTA L1 to IOTA EVM

To send OFT-wrapped tokens back from IOTA L1 to IOTA EVM, use the MoveVM utilities:

```bash
yarn send-oft
```

For detailed instructions, see [MoveVM OFT Send Guide](https://github.com/iota-community/layerzero-move-oft-v2-utils/blob/main/README_OFT_send.md).

## LayerZero Endpoint IDs

| Network          | Endpoint ID |
| ---------------- | ----------- |
| IOTA L1 Mainnet  | 30423       |
| IOTA EVM Mainnet | 30284       |

For a complete list of endpoint IDs, see [LayerZero Deployed Contracts](https://docs.layerzero.network/v2/deployments/deployed-contracts).

## References

- [LayerZero OFT Interface - quoteSend()](https://github.com/LayerZero-Labs/LayerZero-v2/blob/main/oapp/contracts/oft/interfaces/IOFT.sol#L127C60-L127C73)
- [LayerZero OFT Interface - send()](https://github.com/LayerZero-Labs/LayerZero-v2/blob/main/oapp/contracts/oft/interfaces/IOFT.sol#L144)
- [LayerZero OFT Interface - SendParam struct](https://github.com/LayerZero-Labs/LayerZero-v2/blob/main/oapp/contracts/oft/interfaces/IOFT.sol#L10)
- [@layerzerolabs/scan-client](https://www.npmjs.com/package/@layerzerolabs/scan-client#example-usage)
- [LayerZero Explorer](https://layerzeroscan.com/)
