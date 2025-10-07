import { BrowserContext, expect, Page } from '@playwright/test';
import { getExtensionUrl } from './helpers/browser';
import { checkL2IotaBalanceWithRetries } from './helpers/balances';
import { THREE_MINUTES } from './utils/constants';
import { executeBridgeTransaction, setBridgeAmount, toggleBridgeDirection } from './helpers/ui';
import { test } from './helpers/fixtures';

interface TestContext {
    browser: BrowserContext;
    page: Page;
    addressL1: string;
    addressL2: string;
}
test.describe.serial('Deposit then withdraw Iota roundtrip', () => {
    test.setTimeout(THREE_MINUTES);

    let shared: TestContext;

    test.beforeAll('setup L1/L2 wallets', async ({ browserWithBothExtensionsSetup }) => {
        test.setTimeout(THREE_MINUTES);
        const persistentSetup = await browserWithBothExtensionsSetup('depositThenWithdrawIota');
        shared = persistentSetup;
    });

    test('should successfully process an L1 deposit', async () => {
        const { page, browser, addressL2 } = shared;
        const iotaAmountToSend = '3';

        await expect(page.getByText(/Available/i)).toBeVisible({ timeout: 10000 });

        await setBridgeAmount(page, iotaAmountToSend);

        await expect(page.getByText('Bridge Assets')).toBeEnabled({ timeout: 30000 });

        const gasFeeValue = await page
            .locator('div:has(> span:text("Est. IOTA Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(Number(gasFeeValue).toFixed(5)).toEqual('0.00663');

        const gasFeeValueEVM = await page
            .locator('div:has(> span:text("Est. IOTA EVM Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(gasFeeValueEVM).toEqual('0.001');

        await executeBridgeTransaction(page, browser, true);

        const balance = await checkL2IotaBalanceWithRetries(addressL2 ?? '');
        expect(Number(balance)).toEqual(Number(iotaAmountToSend));
    });

    test('should successfully process an L2 deposit', async () => {
        const { page, browser } = shared;
        const iotaAmountToSend = '2';

        await toggleBridgeDirection(page);

        await expect(page.getByText(/Available/i)).toBeVisible({ timeout: 10000 });

        await setBridgeAmount(page, iotaAmountToSend);

        // check est. gas fees and your receive
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

        await expect(pageWithL1WalletExtension.getByTestId('coin-balance')).toHaveText('2.99', {
            timeout: THREE_MINUTES,
        });
        await pageWithL1WalletExtension.close();
    });
});
