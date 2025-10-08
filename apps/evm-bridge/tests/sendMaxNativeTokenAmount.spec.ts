import { expect } from '@playwright/test';
import {
    checkL2CoinBalanceForAddressWithRetries,
    checkL1CoinBalanceForAddressWithRetries,
} from './helpers/balances';
import { THREE_MINUTES, TOOL_COIN_TYPE } from './utils/constants';
import {
    clickMaxAmount,
    executeBridgeTransaction,
    selectCoin,
    waitForToastMessage,
} from './helpers/ui';
import { test } from './helpers/fixtures';

test.describe('Send MAX native token amount from L1', () => {
    test.describe.configure({ timeout: THREE_MINUTES });

    test('should bridge successfully', async ({ browserWithL1Setup }) => {
        const setup = await browserWithL1Setup('sendMaxNativeTokenAmountL1');
        const { browser, page, addressL2 } = setup;
        const nativeTokenAmount = '3';

        await selectCoin(page, 'Tool');

        await expect(page.getByText(/Available/i)).toBeVisible({ timeout: 10000 });

        await clickMaxAmount(page);

        const amountField = page.getByTestId('bridge-amount');
        await expect(amountField).toBeVisible();
        await expect(amountField).toHaveValue(nativeTokenAmount);

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

        const balance = await checkL2CoinBalanceForAddressWithRetries(addressL2, TOOL_COIN_TYPE);
        expect(balance).toEqual(nativeTokenAmount);
    });
});

test.describe('Send MAX native token amount from L2', () => {
    test.describe.configure({ timeout: THREE_MINUTES });

    test('should bridge successfully', async ({ browserWithL2Setup }) => {
        const setup = await browserWithL2Setup('sendMaxNativeTokenAmountL2');
        const { browser, page, addressL1 } = setup;
        const nativeTokenAmount = '3';

        await selectCoin(page, 'Tool');

        await expect(page.getByText(/Available/i)).toBeVisible({ timeout: 10000 });

        await clickMaxAmount(page);

        const amountField = page.getByTestId('bridge-amount');
        await expect(amountField).toBeVisible();
        await expect(amountField).toHaveValue(nativeTokenAmount);

        // check est. gas fees and your receive
        await expect(page.getByText('Bridge Assets')).toBeEnabled({ timeout: 30000 });

        const gasFeeValue = await page
            .locator('div:has(> span:text("Est. IOTA EVM Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(Number(gasFeeValue).toFixed(6)).toMatch(/^0\.0003\d\d$/);

        await executeBridgeTransaction(page, browser, false);
        await waitForToastMessage(
            page,
            'Withdraw transaction confirmed! Your funds have been transferred.',
        );

        const l1Balance = await checkL1CoinBalanceForAddressWithRetries(addressL1, TOOL_COIN_TYPE);
        expect(l1Balance).toEqual(nativeTokenAmount);
    });
});
