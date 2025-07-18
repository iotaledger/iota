import { BrowserContext, expect, Page } from '@playwright/test';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { closeBrowserTabsExceptLast, test } from './helpers/browser';
import { checkL2IotaBalanceWithRetries, checkL1IotaBalanceWithRetries } from './helpers/balances';
import { addL1FundsThroughBridgeUI, fundL2AddressWithIscClient } from './helpers/transactions';
import {
    createL1Wallet,
    getRandomL2MnemonicAndAddress,
    createL2Wallet,
    addNetworkToMetaMask,
    connectL1Wallet,
    connectL2Wallet,
} from './helpers/wallet';
import { THREE_MINUTES } from './utils/constants';
import {
    clickMaxAmount,
    executeBridgeTransaction,
    setReceiverAddress,
    toggleBridgeDirection,
} from './helpers/ui';

test.describe('Send MAX Iota amount from L1', () => {
    test.describe.configure({ timeout: THREE_MINUTES });

    let browserL1: BrowserContext;
    let testPageL1: Page;

    test.beforeAll('setup L1 wallet', async ({ contextL1, l1ExtensionUrl }) => {
        test.setTimeout(THREE_MINUTES);

        testPageL1 = await contextL1.newPage();
        await createL1Wallet(testPageL1, l1ExtensionUrl);

        testPageL1 = await contextL1.newPage();
        browserL1 = contextL1;
        await closeBrowserTabsExceptLast(browserL1);

        await testPageL1.goto('/');

        await connectL1Wallet(testPageL1, browserL1);

        const { address: addressL2 } = getRandomL2MnemonicAndAddress();

        await setReceiverAddress(testPageL1, addressL2);
    });

    test('should bridge successfully', async () => {
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

        const addressL2 = await testPageL1.getByTestId('receive-address').inputValue();
        const balance = await checkL2IotaBalanceWithRetries(addressL2);

        expect(balance).toEqual('9.990388');
    });
});

test.describe('Send MAX Iota amount from L2', () => {
    test.describe.configure({ timeout: THREE_MINUTES });

    let browserL2: BrowserContext;
    let testPageL2: Page;

    test.beforeAll('setup L2 wallet', async ({ contextL2, l2ExtensionUrl }) => {
        test.setTimeout(THREE_MINUTES);

        testPageL2 = await contextL2.newPage();

        await createL2Wallet(testPageL2, l2ExtensionUrl);

        await addNetworkToMetaMask(testPageL2);

        testPageL2 = await contextL2.newPage();
        browserL2 = contextL2;
        await closeBrowserTabsExceptLast(browserL2);
        await testPageL2.goto('/');

        await connectL2Wallet(testPageL2, browserL2);

        const l2WalletConnectedButton = testPageL2.getByRole('button', {
            name: /Dropdown/,
        });

        await expect(l2WalletConnectedButton).toBeVisible();
        const balanceL2Display = l2WalletConnectedButton.getByText('0 IOTA');
        await expect(balanceL2Display).toBeVisible();

        await toggleBridgeDirection(testPageL2);

        const keypair = new Ed25519Keypair();
        const addressL1 = keypair.toIotaAddress();

        await setReceiverAddress(testPageL2, addressL1);
    });

    test('should bridge successfully', async () => {
        const addressL2 = await testPageL2.getByTestId('sender-address').inputValue();
        const iotaAmount = 9;
        await fundL2AddressWithIscClient(addressL2, iotaAmount);

        const balance = await checkL2IotaBalanceWithRetries(addressL2);
        expect(Number(balance)).toEqual(iotaAmount);

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

        const addressL1 = await testPageL2.getByTestId('receive-address').inputValue();
        const l1Balance = await checkL1IotaBalanceWithRetries(addressL1);
        expect(Number(l1Balance).toFixed(6)).toMatch(/^8\.9996\d\d$/);
    });
});
