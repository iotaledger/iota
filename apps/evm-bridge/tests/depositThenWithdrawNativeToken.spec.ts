import { BrowserContext, expect, Page } from '@playwright/test';
import {
    checkL1CoinBalanceForAddressWithRetries,
    checkL2CoinBalanceForAddressWithRetries,
} from './helpers/balances';
import { THREE_MINUTES, TOOL_COIN_TYPE } from './utils/constants';
import {
    executeBridgeTransaction,
    selectCoin,
    setBridgeAmount,
    toggleBridgeDirection,
} from './helpers/ui';
import { test } from './helpers/fixtures';

interface TestContext {
    browser: BrowserContext;
    page: Page;
    addressL1: string;
    addressL2: string;
}

test.describe.serial('Deposit then withdraw native tokens roundtrip', () => {
    test.setTimeout(THREE_MINUTES);

    let shared: TestContext;

    test.beforeAll('setup L1/L2 wallets', async ({ roundtripNativeTokenSetup }) => {
        const persistentSetup = await roundtripNativeTokenSetup('depositThenWithdrawNativeToken');
        shared = persistentSetup;
    });

    test('should successfully process an L1 deposit', async () => {
        const { page, browser, addressL1, addressL2 } = shared;
        const nativeTokenAmount = 3;

        const l1CoinBalance = await checkL1CoinBalanceForAddressWithRetries(
            addressL1 ?? '',
            TOOL_COIN_TYPE,
        );
        expect(Number(l1CoinBalance)).toBeGreaterThan(nativeTokenAmount);

        await selectCoin(page, 'Tool');

        await setBridgeAmount(page, nativeTokenAmount);
        // check est. gas fees and your receive
        await expect(page.getByText('Bridge Assets')).toBeEnabled({ timeout: 10000 });

        const gasFeeValue = await page
            .locator('div:has(> span:text("Est. IOTA Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        const gasFeeFixed = Number(Number(gasFeeValue).toFixed(3));
        expect(gasFeeFixed).toBeGreaterThanOrEqual(0.008);
        expect(gasFeeFixed).toBeLessThanOrEqual(0.01);

        const gasFeeValueEVM = await page
            .locator('div:has(> span:text("Est. IOTA EVM Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(gasFeeValueEVM).toEqual('0.001');

        await executeBridgeTransaction(page, browser, true);

        const balance = await checkL2CoinBalanceForAddressWithRetries(
            addressL2 ?? '',
            TOOL_COIN_TYPE,
        );
        expect(balance).toEqual(nativeTokenAmount.toString());
    });

    test('should successfully process an L2 deposit', async () => {
        const { page, browser, addressL1 } = shared;
        const nativeTokenAmount = '2';

        await toggleBridgeDirection(page);

        await selectCoin(page, 'Tool');

        await setBridgeAmount(page, nativeTokenAmount);

        // check est. gas fees and your receive
        await expect(page.getByText('Bridge Assets')).toBeEnabled({ timeout: 10000 });

        const gasFeeValue = await page
            .locator('div:has(> span:text("Est. IOTA EVM Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(Number(gasFeeValue).toFixed(6)).toMatch(/^0\.0003\d\d$/);

        await executeBridgeTransaction(page, browser, false);
        await page.waitForTimeout(2500);
        // Check funds on L1 wallet
        const l1Balance = await checkL1CoinBalanceForAddressWithRetries(
            addressL1 ?? '',
            TOOL_COIN_TYPE,
        );
        expect(l1Balance).toEqual('3');
    });

    test.afterAll(async () => {
        // Important: Close persistent context manually when done
        // await shared.browser.close().catch((e) => console.error('Error closing browser:', e));
    });
});
