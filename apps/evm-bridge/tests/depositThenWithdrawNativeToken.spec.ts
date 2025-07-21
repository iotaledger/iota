import { BrowserContext, expect, Page } from '@playwright/test';
import { generate24WordMnemonic, deriveAddressFromMnemonic } from './utils/utils';
import { closeBrowserTabsExceptLast, test } from './helpers/browser';
import {
    checkL1CoinBalanceForAddressWithRetries,
    checkL2CoinBalanceForAddressWithRetries,
} from './helpers/balances';
import {
    addL1FundsThroughBridgeUI,
    fundL1AddressWithNativeTokens,
    fundL2AddressWithIscClient,
} from './helpers/transactions';
import {
    importL1WalletFromMnemonic,
    createL2Wallet,
    addNetworkToMetaMask,
    connectL1Wallet,
    connectL2Wallet,
} from './helpers/wallet';
import { THREE_MINUTES, TOOL_COIN_TYPE } from './utils/constants';
import {
    executeBridgeTransaction,
    selectCoin,
    setBridgeAmount,
    setReceiverAddress,
    toggleBridgeDirection,
} from './helpers/ui';

test.describe.serial('Deposit then withdraw native tokens roundtrip', () => {
    test.setTimeout(THREE_MINUTES);

    let browserL1: BrowserContext;
    let browserL2: BrowserContext;
    let pageWithL1Wallet: Page;
    let pageWithL2Wallet: Page;
    let addressL1: string | null = null;
    let addressL2: string | null = null;

    test.beforeAll(
        'setup L1/L2 wallets',
        async ({ contextL1, l1ExtensionUrl, contextL2, l2ExtensionUrl }) => {
            test.setTimeout(THREE_MINUTES);

            // Create L1 wallet on pageWithL1Wallet
            const mnemonicL1 = generate24WordMnemonic();

            pageWithL1Wallet = await contextL1.newPage();
            await importL1WalletFromMnemonic(pageWithL1Wallet, l1ExtensionUrl, mnemonicL1);

            addressL1 = deriveAddressFromMnemonic(mnemonicL1);

            // Create L2 wallet on pageWithL2Wallet
            pageWithL2Wallet = await contextL2.newPage();
            addressL2 = await createL2Wallet(pageWithL2Wallet, l2ExtensionUrl);

            browserL1 = contextL1;
            browserL2 = contextL2;

            await addNetworkToMetaMask(pageWithL2Wallet);

            await pageWithL1Wallet.goto('/');
            await pageWithL2Wallet.goto('/');

            await closeBrowserTabsExceptLast(browserL1);
            await closeBrowserTabsExceptLast(browserL2);

            if (addressL1 === null || addressL2 === null) {
                throw new Error('L1 or L2 address not found');
            }

            // Connect L1 wallet to the EVM Bridge on pageWithL1Wallet
            await connectL1Wallet(pageWithL1Wallet, browserL1);

            // Manually input addressL2 on pageWithL1Wallet
            await setReceiverAddress(pageWithL1Wallet, addressL2);

            // Connect L2 wallet to the EVM Bridge on pageWithL2Wallet
            await connectL2Wallet(pageWithL2Wallet, browserL2);

            // Switch to the L2 wallet page on pageWithL2Wallet
            await toggleBridgeDirection(pageWithL2Wallet);

            // Manually input addressL1 on pageWithL2Wallet
            await setReceiverAddress(pageWithL2Wallet, addressL1);

            // Fund L1 wallet with IOTA and native tokens
            await addL1FundsThroughBridgeUI(pageWithL1Wallet, browserL1);
            const nativeTokenAmount = 5;
            await fundL1AddressWithNativeTokens(addressL1, nativeTokenAmount);
            // Fund L2 wallet with IOTA
            await fundL2AddressWithIscClient(addressL2, 5);
        },
    );

    test('should successfully process an L1 deposit', async () => {
        const nativeTokenAmount = 3;

        const l1CoinBalance = await checkL1CoinBalanceForAddressWithRetries(
            addressL1 ?? '',
            TOOL_COIN_TYPE,
        );
        expect(Number(l1CoinBalance)).toBeGreaterThan(nativeTokenAmount);

        await selectCoin(pageWithL1Wallet, 'Tool');

        await setBridgeAmount(pageWithL1Wallet, nativeTokenAmount);

        // check est. gas fees and your receive
        await pageWithL1Wallet.waitForTimeout(2500);

        const gasFeeValue = await pageWithL1Wallet
            .locator('div:has(> span:text("Est. IOTA Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        const gasFeeFixed = Number(Number(gasFeeValue).toFixed(3));
        expect(gasFeeFixed).toBeGreaterThanOrEqual(0.008);
        expect(gasFeeFixed).toBeLessThanOrEqual(0.01);

        const gasFeeValueEVM = await pageWithL1Wallet
            .locator('div:has(> span:text("Est. IOTA EVM Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(gasFeeValueEVM).toEqual('0.001');

        await executeBridgeTransaction(pageWithL1Wallet, browserL1, true);

        const balance = await checkL2CoinBalanceForAddressWithRetries(
            addressL2 ?? '',
            TOOL_COIN_TYPE,
        );
        expect(balance).toEqual(nativeTokenAmount.toString());
    });

    test('should successfully process an L2 deposit', async () => {
        const nativeTokenAmount = '2';

        await selectCoin(pageWithL2Wallet, 'Tool');

        await setBridgeAmount(pageWithL2Wallet, nativeTokenAmount);

        // check est. gas fees and your receive
        await pageWithL2Wallet.waitForTimeout(2500);

        const gasFeeValue = await pageWithL2Wallet
            .locator('div:has(> span:text("Est. IOTA EVM Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(Number(gasFeeValue).toFixed(6)).toMatch(/^0\.0003\d\d$/);

        await executeBridgeTransaction(pageWithL2Wallet, browserL2, false);

        // Check funds on L1 wallet
        const l1Balance = await checkL1CoinBalanceForAddressWithRetries(
            addressL1 ?? '',
            TOOL_COIN_TYPE,
        );
        expect(l1Balance).toEqual(nativeTokenAmount.toString());
    });
});
