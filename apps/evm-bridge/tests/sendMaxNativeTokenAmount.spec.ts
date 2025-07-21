import { expect } from '@playwright/test';
import {
    checkL2CoinBalanceForAddressWithRetries,
    checkL1CoinBalanceForAddressWithRetries,
} from './helpers/balances';
import {
    addL1FundsThroughBridgeUI,
    fundL1AddressWithNativeTokens,
    fundL2AddressWithIscClient,
} from './helpers/transactions';
import { THREE_MINUTES, TOOL_COIN_TYPE } from './utils/constants';
import { clickMaxAmount, executeBridgeTransaction, selectCoin } from './helpers/ui';
import { test } from './helpers/fixtures';

test.describe('Send MAX native token amount from L1', () => {
    test.describe.configure({ timeout: THREE_MINUTES });

    test('should bridge successfully', async ({ l1Setup }) => {
        const { browser: browserL1, page: testPageL1, receiverAddress } = l1Setup;
        const addressL1 = await testPageL1.getByTestId('sender-address').inputValue();
        const nativeTokenAmount = 2;

        await addL1FundsThroughBridgeUI(testPageL1, browserL1);
        await fundL1AddressWithNativeTokens(addressL1, nativeTokenAmount);

        // todo: add check for balance with retries instead of wait
        await testPageL1.waitForTimeout(500);

        await selectCoin(testPageL1, 'Tool');

        await clickMaxAmount(testPageL1);

        const amountField = testPageL1.getByTestId('bridge-amount');
        await expect(amountField).toBeVisible();
        await expect(amountField).toHaveValue(nativeTokenAmount.toString());

        // check est. gas fees and your receive
        await testPageL1.waitForTimeout(2500);

        const gasFeeValue = await testPageL1
            .locator('div:has(> span:text("Est. IOTA Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        const gasFeeFixed = Number(Number(gasFeeValue).toFixed(3));
        expect(gasFeeFixed).toBeGreaterThanOrEqual(0.008);
        expect(gasFeeFixed).toBeLessThanOrEqual(0.01);

        const gasFeeValueEVM = await testPageL1
            .locator('div:has(> span:text("Est. IOTA EVM Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(gasFeeValueEVM).toEqual('0.001');

        await executeBridgeTransaction(testPageL1, browserL1, true);

        const balance = await checkL2CoinBalanceForAddressWithRetries(
            receiverAddress,
            TOOL_COIN_TYPE,
        );

        expect(balance).toEqual(nativeTokenAmount.toString());
    });
});

test.describe('Send MAX native token amount from L2', () => {
    test.describe.configure({ timeout: THREE_MINUTES });

    test('should bridge successfully', async ({ l2Setup }) => {
        const { browser: browserL2, page: testPageL2, receiverAddress } = l2Setup;
        const addressL2 = await testPageL2.getByTestId('sender-address').inputValue();
        const nativeTokenAmount = 2;

        await fundL2AddressWithIscClient(addressL2, 9);
        await fundL2AddressWithIscClient(addressL2, nativeTokenAmount, TOOL_COIN_TYPE);

        const nativeTokenBalance = await checkL2CoinBalanceForAddressWithRetries(
            addressL2,
            TOOL_COIN_TYPE,
        );
        expect(nativeTokenBalance).toEqual(nativeTokenAmount.toString());

        await testPageL2.waitForTimeout(500);

        await selectCoin(testPageL2, 'Tool');

        await clickMaxAmount(testPageL2);

        const amountField = testPageL2.getByTestId('bridge-amount');
        await expect(amountField).toBeVisible();
        await expect(amountField).toHaveValue(nativeTokenAmount.toString());

        // check est. gas fees and your receive
        await testPageL2.waitForTimeout(2500);

        const gasFeeValue = await testPageL2
            .locator('div:has(> span:text("Est. IOTA EVM Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(Number(gasFeeValue).toFixed(6)).toMatch(/^0\.0003\d\d$/);

        await executeBridgeTransaction(testPageL2, browserL2, false);

        const l1Balance = await checkL1CoinBalanceForAddressWithRetries(
            receiverAddress,
            TOOL_COIN_TYPE,
        );

        expect(l1Balance).toEqual(nativeTokenAmount.toString());
    });
});
