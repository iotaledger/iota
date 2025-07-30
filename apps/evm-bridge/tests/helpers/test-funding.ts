// Create a new file: helpers/test-funding.ts

import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import {
    sendIotaToAddress,
    fundL1AddressWithNativeTokens,
    fundL2AddressWithIscClient,
} from './transactions';
import { TOOL_COIN_TYPE } from '../utils/constants';

/**
 * Fund wallets for the sendMaxIota test suite
 * @param globalAddress Global funding address
 * @param globalKeypair Global funding keypair
 * @param addressL1 Wallet address l1 for the test
 * @param addressL2 Wallet address l2 for the test
 */
export async function fundSendMaxIotaTestWallets(
    globalAddress: string,
    globalKeypair: Ed25519Keypair,
    addressL1: string,
    addressL2: string,
): Promise<void> {
    console.log('📝 Funding sendMaxIota test wallets...');

    // Send IOTA to L1 address for testing max send
    await sendIotaToAddress(
        globalAddress,
        globalKeypair,
        addressL1,
        2, // Amount of IOTA
    );

    // Fund L2 address with IOTA
    await fundL2AddressWithIscClient(globalAddress, globalKeypair, addressL2, 2);

    console.log('✅ sendMaxIota test wallets funded successfully');
}

/**
 * Fund wallets for the sendMaxNativeToken test suite
 * @param globalAddress Global funding address
 * @param globalKeypair Global funding keypair
 * @param toolCoinAddress Tool coin source address
 * @param toolCoinKeypair Tool coin keypair
 * @param addressL1 Wallet address l1 for the test
 * @param addressL2 Wallet address l2 for the test
 */
export async function fundSendMaxNativeTokenTestWallets(
    globalAddress: string,
    globalKeypair: Ed25519Keypair,
    toolCoinAddress: string,
    toolCoinKeypair: Ed25519Keypair,
    addressL1: string,
    addressL2: string,
): Promise<void> {
    console.log('📝 Funding sendMaxNativeToken test wallets...');

    // Send IOTA to L1 address for gas
    await sendIotaToAddress(globalAddress, globalKeypair, addressL1, 0.5);

    // Send tool coin to L1 address
    await fundL1AddressWithNativeTokens(toolCoinAddress, toolCoinKeypair, addressL1, 3);

    // Send IOTA to L2 address for gas
    await fundL2AddressWithIscClient(globalAddress, globalKeypair, addressL2, 1);

    // Send tool coin to L2 address
    await fundL2AddressWithIscClient(
        toolCoinAddress,
        toolCoinKeypair,
        addressL2,
        3,
        TOOL_COIN_TYPE,
    );

    console.log('✅ sendMaxNativeToken test wallets funded successfully');
}

/**
 * Fund wallets for the depositThenWithdrawIota test suite
 * @param globalAddress Global funding address
 * @param globalKeypair Global funding keypair
 * @param toolCoinAddress Tool coin source address
 * @param toolCoinKeypair Tool coin keypair
 * @param addressL1 Wallet address l1 for the test
 * @param addressL2 Wallet address l2 for the test
 */
export async function fundDepostiThenWithdrawIotaTestWallets(
    globalAddress: string,
    globalKeypair: Ed25519Keypair,
    addressL1: string,
): Promise<void> {
    console.log('📝 Funding depositThenWithdrawIota test wallets...');

    // Send IOTA to L1 address for gas
    await sendIotaToAddress(
        globalAddress,
        globalKeypair,
        addressL1,
        4, // Amount of IOTA
    );
    console.log('✅ depositThenWithdrawIota test wallets funded successfully');
}

/**
 * Fund wallets for the depositThenWithdrawNativeToken test suite
 * @param globalAddress Global funding address
 * @param globalKeypair Global funding keypair
 * @param toolCoinAddress Tool coin source address
 * @param toolCoinKeypair Tool coin keypair
 * @param addressL1 Wallet address l1 for the test
 * @param addressL2 Wallet address l2 for the test
 */
export async function fundDepostiThenWithdrawNativeTokenTestWallets(
    globalAddress: string,
    globalKeypair: Ed25519Keypair,
    toolCoinAddress: string,
    toolCoinKeypair: Ed25519Keypair,
    addressL1: string,
    addressL2: string,
): Promise<void> {
    console.log('📝 Funding depositThenWithdrawNativeToken test wallets...');

    // Send IOTA to L1 address for gas
    await sendIotaToAddress(
        globalAddress,
        globalKeypair,
        addressL1,
        0.5, // Amount of IOTA
    );

    // Send tool coin to L1 address
    await fundL1AddressWithNativeTokens(toolCoinAddress, toolCoinKeypair, addressL1, 4);

    // Send IOTA to L2 address for gas
    await fundL2AddressWithIscClient(globalAddress, globalKeypair, addressL2, 1);

    console.log('✅ depositThenWithdrawNativeToken test wallets funded successfully');
}
