import { expect } from '@playwright/test';
import { checkL2IotaBalanceWithRetries, checkL1IotaBalanceWithRetries } from './helpers/balances';
import { addL1FundsThroughBridgeUI, fundL2AddressWithIscClient } from './helpers/transactions';
import { THREE_MINUTES } from './utils/constants';
import { clickMaxAmount, executeBridgeTransaction } from './helpers/ui';
import { test } from './helpers/fixtures';

test.describe('Send MAX Iota amount from L1', () => {
    test.describe.configure({ timeout: THREE_MINUTES });

    test('should bridge successfully', async ({ l1Setup }) => {
        const { browser: browserL1, page: testPageL1, receiverAddress } = l1Setup;

        await addL1FundsThroughBridgeUI(testPageL1, browserL1);

        // wait for available todo reomove wait for check balance with retries
        await testPageL1.waitForTimeout(2500);

        await clickMaxAmount(testPageL1);

        const amountField = testPageL1.getByTestId('bridge-amount');
        await expect(amountField).toBeVisible();
        await expect(amountField).toHaveValue('~ 9.990388');

        // check est. gas fees and your receive
        await testPageL1.waitForTimeout(2500);

        const gasFeeValue = await testPageL1
            .locator('div:has(> span:text("Est. IOTA Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(Number(gasFeeValue).toFixed(5)).toEqual('0.00663');

        const gasFeeValueEVM = await testPageL1
            .locator('div:has(> span:text("Est. IOTA EVM Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(gasFeeValueEVM).toEqual('0.001');

        await executeBridgeTransaction(testPageL1, browserL1, true);

        const balance = await checkL2IotaBalanceWithRetries(receiverAddress);

        expect(balance).toEqual('9.990388');
    });
});

test.describe('Send MAX Iota amount from L2', () => {
    test.describe.configure({ timeout: THREE_MINUTES });

    test('should bridge successfully', async ({ l2Setup }) => {
        const { browser: browserL2, page: testPageL2, receiverAddress } = l2Setup;

        const addressL2 = await testPageL2.getByTestId('sender-address').inputValue();
        const iotaAmount = 9;
        await fundL2AddressWithIscClient(addressL2, iotaAmount);

        const balance = await checkL2IotaBalanceWithRetries(addressL2);
        expect(Number(balance)).toEqual(iotaAmount);

        // check est. gas fees and your receive
        await testPageL2.waitForTimeout(2500);

        await clickMaxAmount(testPageL2);

        const amountField = testPageL2.getByTestId('bridge-amount');
        await expect(amountField).toBeVisible();
        await expect(amountField).toHaveValue(/~ 8\.9996[0-9]*/);

        // check est. gas fees and your receive
        await testPageL2.waitForTimeout(2500);

        const gasFeeValue = await testPageL2
            .locator('div:has(> span:text("Est. IOTA EVM Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(Number(gasFeeValue).toFixed(6)).toMatch(/^0\.0003\d\d$/);

        await executeBridgeTransaction(testPageL2, browserL2, false);

        const l1Balance = await checkL1IotaBalanceWithRetries(receiverAddress);
        expect(Number(l1Balance).toFixed(6)).toMatch(/^8\.9996\d\d$/);
    });
});
