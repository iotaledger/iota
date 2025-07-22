import { BrowserContext, expect, Page } from '@playwright/test';
import {
    checkL1CoinBalanceForAddressWithRetries,
    checkL2CoinBalanceForAddressWithRetries,
} from './helpers/balances';
import { THREE_MINUTES, TOOL_COIN_TYPE } from './utils/constants';
import { executeBridgeTransaction, selectCoin, setBridgeAmount } from './helpers/ui';
import { test } from './helpers/fixtures';
import {
    addL1FundsThroughBridgeUI,
    fundL1AddressWithNativeTokens,
    fundL2AddressWithIscClient,
} from './helpers/transactions';
import { closeBrowserTabsExceptLast } from './helpers/browser';

interface TestContext {
    pageWithL1Wallet: Page;
    pageWithL2Wallet: Page;
    browserL1: BrowserContext;
    browserL2: BrowserContext;
    addressL1: string;
    addressL2: string;
}

test.describe.serial('Deposit then withdraw native tokens roundtrip', () => {
    test.setTimeout(THREE_MINUTES);

    let shared: TestContext;

    test.beforeAll('setup L1/L2 wallets', async ({ roundtripSetup }) => {
        if (!roundtripSetup) {
            throw new Error('roundtripSetup fixture not available');
        }

        shared = roundtripSetup;
        // Fund wallets
        await addL1FundsThroughBridgeUI(shared.pageWithL1Wallet, shared.browserL1);
        await fundL1AddressWithNativeTokens(shared.addressL1, 5);
        await fundL2AddressWithIscClient(shared.addressL2, 5);
    });

    test('should successfully process an L1 deposit', async () => {
        const { pageWithL1Wallet, browserL1, addressL1, addressL2 } = shared;
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

        await closeBrowserTabsExceptLast(browserL1);
    });

    test('should successfully process an L2 deposit', async () => {
        const { pageWithL2Wallet, browserL2, addressL1 } = shared;
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
