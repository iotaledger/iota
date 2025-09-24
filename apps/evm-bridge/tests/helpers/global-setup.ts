import { existsSync, writeFileSync, mkdirSync } from 'fs';
import {
    generate24WordMnemonic,
    deriveAddressFromMnemonic,
    generateTestWallets,
} from '../utils/utils';
import { STATE_FILE, STATE_DIR } from './paths';
import { requestFundsFromFaucet } from './transactions';
import { MNEMONIC_TOOL_COIN } from '../utils/constants';
import {
    fundDepostiThenWithdrawIotaTestWallets,
    fundDepostiThenWithdrawNativeTokenTestWallets,
    fundSendMaxIotaTestWallets,
    fundSendMaxNativeTokenTestWallets,
    getTotalFundingUsage,
} from './test-funding';
import { WalletState } from './shared-state';

async function globalSetup() {
    if (!existsSync(STATE_DIR)) mkdirSync(STATE_DIR, { recursive: true });

    // Generate test addresses and mnemonics
    const globalMnemonicL1 = generate24WordMnemonic();
    const { address: globalAddressL1, keypair: globalKeypair } =
        deriveAddressFromMnemonic(globalMnemonicL1);

    const { address: toolCoinAddress, keypair: toolCoinKeypair } =
        deriveAddressFromMnemonic(MNEMONIC_TOOL_COIN);

    const sendMaxIotaWalletsL1 = generateTestWallets();
    const sendMaxIotaWalletsL2 = generateTestWallets();

    const sendMaxNativeTokensWalletsL1 = generateTestWallets();
    const sendMaxNativeTokensWalletsL2 = generateTestWallets();

    const roundTripIotaWallets = generateTestWallets();

    const roundTripNativeTokenWallets = generateTestWallets();

    const state: WalletState = {
        global: {
            addressL1: globalAddressL1,
            mnemonicL1: globalMnemonicL1,
        },
        tests: {
            sendMaxIotaAmountL1: sendMaxIotaWalletsL1,
            sendMaxIotaAmountL2: sendMaxIotaWalletsL2,
            sendMaxNativeTokenAmountL1: sendMaxNativeTokensWalletsL1,
            sendMaxNativeTokenAmountL2: sendMaxNativeTokensWalletsL2,
            depositThenWithdrawIota: roundTripIotaWallets,
            depositThenWithdrawNativeToken: roundTripNativeTokenWallets,
        },
    };

    writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));

    // Fund global wallet from faucet
    await requestFundsFromFaucet(globalAddressL1);
    await requestFundsFromFaucet(globalAddressL1);

    const { totalIota, totalTool } = getTotalFundingUsage();
    console.log('💰 Test Funding Summary:');
    console.log(`   Total IOTA required: ${totalIota}`);
    console.log(`   Total TOOL required: ${totalTool}`);
    // Fund Test Addresses
    await fundSendMaxIotaTestWallets(
        globalAddressL1,
        globalKeypair,
        sendMaxIotaWalletsL1.addressL1,
        sendMaxIotaWalletsL2.addressL2,
    );

    await fundSendMaxNativeTokenTestWallets(
        globalAddressL1,
        globalKeypair,
        toolCoinAddress,
        toolCoinKeypair,
        sendMaxNativeTokensWalletsL1.addressL1,
        sendMaxNativeTokensWalletsL2.addressL2,
    );

    await fundDepostiThenWithdrawIotaTestWallets(
        globalAddressL1,
        globalKeypair,
        roundTripIotaWallets.addressL1,
    );

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
