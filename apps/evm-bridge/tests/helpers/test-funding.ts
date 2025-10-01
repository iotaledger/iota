import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import {
    sendIotaToAddress,
    fundL1AddressWithNativeTokens,
    fundL2AddressWithIscClient,
} from './transactions';
import { TOOL_COIN_TYPE } from '../utils/constants';

/**
 * Shared funding state configuration
 * Centralized place to manage funding amounts for tests
 */
export const FUNDING_AMOUNTS = {
    // SendMaxIota test funding
    sendMaxIota: {
        l1Iota: 2, // IOTA to fund L1 address
        l2Iota: 2, // IOTA to fund L2 address
    },

    // SendMaxNativeToken test funding
    sendMaxNativeToken: {
        l1Iota: 0.5, // IOTA for gas on L1
        l1Tool: 3, // TOOL tokens on L1
        l2Iota: 1, // IOTA for gas on L2
        l2Tool: 3, // TOOL tokens on L2
    },

    // DepositThenWithdrawIota test funding
    depositWithdrawIota: {
        l1Iota: 4, // IOTA on L1
    },

    // DepositThenWithdrawNativeToken test funding
    depositWithdrawNativeToken: {
        l1Iota: 0.5, // IOTA for gas on L1
        l1Tool: 4, // TOOL tokens on L1
        l2Iota: 1, // IOTA for gas on L2
    },
};

/**
 * Calculate and return the total funding amounts across all tests
 * @returns Object with total IOTA and TOOL usage
 */
export function getTotalFundingUsage(): { totalIota: number; totalTool: number } {
    let totalIota = 0;
    let totalTool = 0;

    // Loop through all test configurations
    Object.values(FUNDING_AMOUNTS).forEach((testConfig) => {
        // Add all properties that contain "Iota" in the name
        Object.entries(testConfig).forEach(([key, value]) => {
            if (key.toLowerCase().includes('iota')) {
                totalIota += Number(value);
            } else if (key.toLowerCase().includes('tool')) {
                totalTool += Number(value);
            }
        });
    });

    return { totalIota, totalTool };
}

export async function fundSendMaxIotaTestWallets(
    globalAddress: string,
    globalKeypair: Ed25519Keypair,
    addressL1: string,
    addressL2: string,
): Promise<void> {
    console.log('📝 Funding sendMaxIota test wallets...');
    const { l1Iota, l2Iota } = FUNDING_AMOUNTS.sendMaxIota;

    // Send IOTA to L1 address for testing max send
    await sendIotaToAddress(globalAddress, globalKeypair, addressL1, l1Iota);

    // Fund L2 address with IOTA
    await fundL2AddressWithIscClient(globalAddress, globalKeypair, addressL2, l2Iota);

    console.log('✅ sendMaxIota test wallets funded successfully');
}

export async function fundSendMaxNativeTokenTestWallets(
    globalAddress: string,
    globalKeypair: Ed25519Keypair,
    toolCoinAddress: string,
    toolCoinKeypair: Ed25519Keypair,
    addressL1: string,
    addressL2: string,
): Promise<void> {
    console.log('📝 Funding sendMaxNativeToken test wallets...');
    const { l1Iota, l1Tool, l2Iota, l2Tool } = FUNDING_AMOUNTS.sendMaxNativeToken;

    // Send IOTA to L1 address for gas
    await sendIotaToAddress(globalAddress, globalKeypair, addressL1, l1Iota);

    // Send tool coin to L1 address
    await fundL1AddressWithNativeTokens(toolCoinAddress, toolCoinKeypair, addressL1, l1Tool);

    // Send IOTA to L2 address for gas
    await fundL2AddressWithIscClient(globalAddress, globalKeypair, addressL2, l2Iota);

    // Send tool coin to L2 address
    await fundL2AddressWithIscClient(
        toolCoinAddress,
        toolCoinKeypair,
        addressL2,
        l2Tool,
        TOOL_COIN_TYPE,
    );

    console.log('✅ sendMaxNativeToken test wallets funded successfully');
}

export async function fundDepostiThenWithdrawIotaTestWallets(
    globalAddress: string,
    globalKeypair: Ed25519Keypair,
    addressL1: string,
): Promise<void> {
    console.log('📝 Funding depositThenWithdrawIota test wallets...');
    const { l1Iota } = FUNDING_AMOUNTS.depositWithdrawIota;
    // Send IOTA to L1 address for gas
    await sendIotaToAddress(globalAddress, globalKeypair, addressL1, l1Iota);
    console.log('✅ depositThenWithdrawIota test wallets funded successfully');
}

export async function fundDepostiThenWithdrawNativeTokenTestWallets(
    globalAddress: string,
    globalKeypair: Ed25519Keypair,
    toolCoinAddress: string,
    toolCoinKeypair: Ed25519Keypair,
    addressL1: string,
    addressL2: string,
): Promise<void> {
    console.log('📝 Funding depositThenWithdrawNativeToken test wallets...');
    const { l1Iota, l1Tool, l2Iota } = FUNDING_AMOUNTS.depositWithdrawNativeToken;

    // Send IOTA to L1 address for gas
    await sendIotaToAddress(globalAddress, globalKeypair, addressL1, l1Iota);

    // Send tool coin to L1 address
    await fundL1AddressWithNativeTokens(toolCoinAddress, toolCoinKeypair, addressL1, l1Tool);

    // Send IOTA to L2 address for gas
    await fundL2AddressWithIscClient(globalAddress, globalKeypair, addressL2, l2Iota);

    console.log('✅ depositThenWithdrawNativeToken test wallets funded successfully');
}
