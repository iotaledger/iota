import { expect } from '@playwright/test';
import { checkL2IotaBalanceWithRetries, checkL1IotaBalanceWithRetries } from './helpers/balances';
import { THREE_MINUTES } from './utils/constants';
import { clickMaxAmount, executeBridgeTransaction } from './helpers/ui';
import { test } from './helpers/fixtures';

test.describe('Send MAX Iota amount from L1', () => {
    test.describe.configure({ timeout: THREE_MINUTES });

    test('should bridge successfully', async ({ browserWithL1Setup }) => {
        const setup = await browserWithL1Setup('sendMaxIotaAmountL1');
        const { browser, page, addressL2 } = setup;

        await expect(page.getByText(/Available/i)).toBeVisible({ timeout: 10000 });

        await clickMaxAmount(page);

        const amountField = page.getByTestId('bridge-amount');
        await expect(amountField).toBeVisible();
        await expect(amountField).toHaveValue('~ 1.990388');

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

        const balance = await checkL2IotaBalanceWithRetries(addressL2);
        expect(balance).toEqual('1.990388');
    });
});

test.describe('Send MAX Iota amount from L2', () => {
    test.describe.configure({ timeout: THREE_MINUTES });

    test('should bridge successfully', async ({ browserWithL2Setup }) => {
        const setup = await browserWithL2Setup('sendMaxIotaAmountL2');
        const { browser, page, addressL1 } = setup;

        await expect(page.getByText(/Available/i)).toBeVisible({ timeout: 10000 });

        await clickMaxAmount(page);

        const amountField = page.getByTestId('bridge-amount');
        await expect(amountField).toBeVisible();
        await expect(amountField).toHaveValue(/~ 1\.9996[0-9]*/);

        await expect(page.getByText('Bridge Assets')).toBeEnabled({ timeout: 30000 });

        const gasFeeValue = await page
            .locator('div:has(> span:text("Est. IOTA EVM Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(Number(gasFeeValue).toFixed(6)).toMatch(/^0\.0003\d\d$/);

        await executeBridgeTransaction(page, browser, false);

        const l1Balance = await checkL1IotaBalanceWithRetries(addressL1);
        expect(Number(l1Balance).toFixed(6)).toMatch(/^1\.9996\d\d$/);
    });
});
