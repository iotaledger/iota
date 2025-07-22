import { BrowserContext, expect, Page } from '@playwright/test';
import { closeBrowserTabsExceptLast, getExtensionUrl } from './helpers/browser';
import { checkL2IotaBalanceWithRetries } from './helpers/balances';
import { THREE_MINUTES } from './utils/constants';
import { executeBridgeTransaction, setBridgeAmount } from './helpers/ui';
import { test } from './helpers/fixtures';
import { addL1FundsThroughBridgeUI } from './helpers/transactions';

interface TestContext {
    pageWithL1Wallet: Page;
    pageWithL2Wallet: Page;
    browserL1: BrowserContext;
    browserL2: BrowserContext;
    addressL1: string;
    addressL2: string;
}
test.describe.serial('Deposit then withdraw Iota roundtrip', () => {
    test.setTimeout(THREE_MINUTES);

    let shared: TestContext;

    test.beforeAll('setup L1/L2 wallets', async ({ roundtripSetup }) => {
        if (!roundtripSetup) {
            throw new Error('roundtripSetup fixture not available');
        }

        shared = roundtripSetup;
        // Fund wallets
        await addL1FundsThroughBridgeUI(shared.pageWithL1Wallet, shared.browserL1);
    });

    test('should successfully process an L1 deposit', async () => {
        const { pageWithL1Wallet, browserL1, addressL2 } = shared;
        const iotaAmountToSend = '5';
        await setBridgeAmount(pageWithL1Wallet, iotaAmountToSend);

        // check est. gas fees and your receive
        await pageWithL1Wallet.waitForTimeout(2500);

        const gasFeeValue = await pageWithL1Wallet
            .locator('div:has(> span:text("Est. IOTA Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(Number(gasFeeValue).toFixed(5)).toEqual('0.00663');

        const gasFeeValueEVM = await pageWithL1Wallet
            .locator('div:has(> span:text("Est. IOTA EVM Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(gasFeeValueEVM).toEqual('0.001');

        await executeBridgeTransaction(pageWithL1Wallet, browserL1, true);

        const balance = await checkL2IotaBalanceWithRetries(addressL2 ?? '');
        expect(Number(balance)).toEqual(Number(iotaAmountToSend));

        await closeBrowserTabsExceptLast(browserL1);
    });

    test('should successfully process an L2 deposit', async () => {
        const { pageWithL2Wallet, browserL2, browserL1 } = shared;
        const iotaAmountToSend = '2';

        await pageWithL2Wallet.waitForTimeout(2500);

        await setBridgeAmount(pageWithL2Wallet, iotaAmountToSend);

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
        const pageWithL1WalletExtension = await browserL1.newPage();
        const l1ExtensionUrl = await getExtensionUrl(browserL1);
        await pageWithL1WalletExtension.goto(l1ExtensionUrl, { waitUntil: 'commit' });

        await expect(pageWithL1WalletExtension.getByTestId('coin-balance')).toHaveText('6.99', {
            timeout: THREE_MINUTES,
        });
        await pageWithL1WalletExtension.close();
    });
});
