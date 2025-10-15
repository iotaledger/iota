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
import { getExtensionUrl } from './helpers/browser';

interface TestContext {
    browser: BrowserContext;
    page: Page;
    addressL1: string;
    addressL2: string;
}

test.describe.serial('Deposit then withdraw native tokens roundtrip', () => {
    test.setTimeout(THREE_MINUTES);

    let shared: TestContext;

    test.beforeAll('setup L1/L2 wallets', async ({ browserWithBothExtensionsSetup }) => {
        test.setTimeout(THREE_MINUTES);
        const persistentSetup = await browserWithBothExtensionsSetup(
            'depositThenWithdrawNativeToken',
        );
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

        await expect(page.getByText(/Available/i)).toBeVisible({ timeout: 10000 });

        await selectCoin(page, 'Tool');

        await expect(page.getByText(/Available/i)).toBeVisible({ timeout: 10000 });

        await setBridgeAmount(page, nativeTokenAmount);
        // check est. gas fees and your receive
        await expect(page.getByText('Bridge Assets')).toBeEnabled({ timeout: 30000 });

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
        const { page, browser } = shared;
        const nativeTokenAmount = '2';

        await toggleBridgeDirection(page);

        await selectCoin(page, 'Tool');

        await expect(page.getByText(/Available/i)).toBeVisible({ timeout: 10000 });

        await setBridgeAmount(page, nativeTokenAmount);

        await expect(page.getByText('Bridge Assets')).toBeEnabled({ timeout: 30000 });

        const gasFeeValue = await page
            .locator('div:has(> span:text("Est. IOTA EVM Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(Number(gasFeeValue).toFixed(6)).toMatch(/^0\.0003\d\d$/);

        await executeBridgeTransaction(page, browser, false);

        // Check funds on L1 wallet
        const pageWithL1WalletExtension = await browser.newPage();
        const l1ExtensionUrl = await getExtensionUrl(browser);
        await pageWithL1WalletExtension.goto(l1ExtensionUrl, { waitUntil: 'commit' });
        // Wait for div containing "3 TOOL" text to be visible
        await expect(
            pageWithL1WalletExtension.locator('div', { hasText: '3 TOOL' }).first(),
        ).toBeVisible({
            timeout: THREE_MINUTES,
        });
    });
});
