// global-setup.ts
import { existsSync, writeFileSync, mkdirSync } from 'fs';
import {
    generate24WordMnemonic,
    deriveAddressFromMnemonic,
    generateTestWallets,
} from '../utils/utils';
import { STATE_FILE, STATE_DIR } from './paths';
import { requestFundsFromFaucet, sendIotaToAddress } from './transactions';
import { MNEMONIC_TOOL_COIN } from '../utils/constants';
import {
    fundDepostiThenWithdrawNativeTokenTestWallets,
    fundSendMaxIotaTestWallets,
    fundSendMaxNativeTokenTestWallets,
} from './test-funding';
import { WalletState } from './shared-state';

async function globalSetup() {
    // Create state directories
    if (!existsSync(STATE_DIR)) mkdirSync(STATE_DIR, { recursive: true });

    // 1. Create global funding address for faucet requests
    const globalMnemonicL1 = generate24WordMnemonic();
    const { address: globalAddressL1, keypair: globalKeypair } =
        deriveAddressFromMnemonic(globalMnemonicL1);

    const { address: toolCoinAddress, keypair: toolCoinKeypair } =
        deriveAddressFromMnemonic(MNEMONIC_TOOL_COIN);

    // 1. Generate addresses for sendMaxIotaAmount test
    const sendMaxIotaWalletsL1 = generateTestWallets();
    const sendMaxIotaWalletsL2 = generateTestWallets();

    // 2. Generate addresses for Send Max Native Token Amount test
    const sendMaxNativeTokensWalletsL1 = generateTestWallets();
    const sendMaxNativeTokensWalletsL2 = generateTestWallets();

    // 3. Generate addresses for roundtrip iota  test
    const roundTripIotaWallets = generateTestWallets();

    // 4. Generate addresses for roundtrip native token test
    const roundTripNativeTokenWallets = generateTestWallets();

    // Store all addresses in shared state
    const state: WalletState = {
        // Global addresses
        global: {
            addressL1: globalAddressL1,
            mnemonicL1: globalMnemonicL1,
        },
        // Test-specific addresses
        tests: {
            sendMaxIotaAmountL1: sendMaxIotaWalletsL1,
            sendMaxIotaAmountL2: sendMaxIotaWalletsL2,
            sendMaxNativeTokenAmountL1: sendMaxNativeTokensWalletsL1,
            sendMaxNativeTokenAmountL2: sendMaxNativeTokensWalletsL2,
            depositThenWithdraw: roundTripIotaWallets,
            depositThenWithdrawNativeToken: roundTripNativeTokenWallets,
        },

        createdAt: new Date().toISOString(),
    };

    writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));

    // Fund global wallet from faucet
    await requestFundsFromFaucet(globalAddressL1);
    await requestFundsFromFaucet(globalAddressL1);

    // Fund sendMaxIotaAmount test addresses
    await fundSendMaxIotaTestWallets(
        globalAddressL1,
        globalKeypair,
        sendMaxIotaWalletsL1.addressL1,
        sendMaxIotaWalletsL2.addressL2,
    );

    // Fund sendMaxNativeTokenAmount test addresses
    await fundSendMaxNativeTokenTestWallets(
        globalAddressL1,
        globalKeypair,
        toolCoinAddress,
        toolCoinKeypair,
        sendMaxNativeTokensWalletsL1.addressL1,
        sendMaxNativeTokensWalletsL2.addressL2,
    );

    // Fund round trip iota wallets
    await sendIotaToAddress(
        globalAddressL1,
        globalKeypair,
        roundTripIotaWallets.addressL1,
        4, // Amount of IOTA
    );

    // Fund round trip native token wallets
    await fundDepostiThenWithdrawNativeTokenTestWallets(
        globalAddressL1,
        globalKeypair,
        toolCoinAddress,
        toolCoinKeypair,
        roundTripNativeTokenWallets.addressL1,
        roundTripNativeTokenWallets.addressL2,
    );
}

export default globalSetup;
