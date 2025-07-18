import { BrowserContext, Page } from '@playwright/test';
import { test, expect } from './utils/fixtures';
import { importL1WalletFromMnemonic, createL2Wallet } from './utils/auth';
import {
    generate24WordMnemonic,
    deriveAddressFromMnemonic,
    closeBrowserTabsExceptLast,
    addNetworkToMetaMask,
    addL1FundsThroughBridgeUI,
    fundL1AddressWithNativeTokens,
    TOOL_COIN_TYPE,
    checkL2CoinBalanceForAddressWithRetries,
    checkL1CoinBalanceForAddressWithRetries,
    fundL2AddressWithIscClient,
} from './utils/utils';

const THREE_MINUTES = 180_000;

test.describe.serial('Deposit then withdraw native tokens roundtrip', () => {
    test.setTimeout(THREE_MINUTES);

    let browserL1: BrowserContext;
    let browserL2: BrowserContext;
    let pageWithL1Wallet: Page;
    let pageWithL2Wallet: Page;
    let addressL1: string | null = null;
    let addressL2: string | null = null;

    test.beforeAll(
        'setup L1/L2 wallets',
        async ({ contextL1, l1ExtensionUrl, contextL2, l2ExtensionUrl }) => {
            test.setTimeout(THREE_MINUTES);

            // Create L1 wallet on pageWithL1Wallet
            const mnemonicL1 = generate24WordMnemonic();

            pageWithL1Wallet = await contextL1.newPage();
            await importL1WalletFromMnemonic(pageWithL1Wallet, l1ExtensionUrl, mnemonicL1);

            addressL1 = deriveAddressFromMnemonic(mnemonicL1);

            // Create L2 wallet on pageWithL2Wallet
            pageWithL2Wallet = await contextL2.newPage();
            addressL2 = await createL2Wallet(pageWithL2Wallet, l2ExtensionUrl);

            browserL1 = contextL1;
            browserL2 = contextL2;

            await addNetworkToMetaMask(pageWithL2Wallet);

            await pageWithL1Wallet.goto('/');
            await pageWithL2Wallet.goto('/');

            await closeBrowserTabsExceptLast(browserL1);
            await closeBrowserTabsExceptLast(browserL2);

            if (addressL1 === null || addressL2 === null) {
                throw new Error('L1 or L2 address not found');
            }

            // Connect L1 wallet to the EVM Bridge on pageWithL1Wallet
            const connectButtonIdL1 = 'connect-l1-wallet';
            const connectButtonL1 = await pageWithL1Wallet.waitForSelector(
                `[data-testid="${connectButtonIdL1}"]`,
                {
                    state: 'visible',
                },
            );

            await connectButtonL1.click();
            const approveWalletConnectPage = browserL1.waitForEvent('page');
            await pageWithL1Wallet.getByText('IOTA Wallet').click();

            const walletL1Page = await approveWalletConnectPage;
            await walletL1Page.getByRole('button', { name: 'Continue' }).click();
            await walletL1Page.getByRole('button', { name: 'Connect' }).click();

            // Manualy input adressL2 on pageWithL1Wallet
            const toggleManualInputL1 = pageWithL1Wallet.getByTestId(
                'toggle-receiver-address-input',
            );
            await expect(toggleManualInputL1).toBeVisible();
            await toggleManualInputL1.click();

            const addressFieldL1 = pageWithL1Wallet.getByTestId('receive-address');
            await expect(addressFieldL1).toBeVisible();
            addressFieldL1.fill(addressL2);

            // Connect L2 wallet to the EVM Bridge on pageWithL2Wallet
            const connectButtonIdL2 = 'connect-l2-wallet';
            const connectButtonL2 = await pageWithL2Wallet.waitForSelector(
                `[data-testid="${connectButtonIdL2}"]`,
                {
                    state: 'visible',
                },
            );

            await connectButtonL2.click();
            const approveWalletL2ConnectDialog = browserL2.waitForEvent('page');
            await pageWithL2Wallet.getByTestId(/metamask/).click();

            const walletL2Modal = await approveWalletL2ConnectDialog;
            await walletL2Modal.waitForLoadState();
            await walletL2Modal.getByRole('button', { name: 'Connect' }).click();

            // Switch to the L2 wallet page on pageWithL2Wallet
            const toggleBridgeDirectionButton =
                pageWithL2Wallet.getByTestId('toggle-bridge-direction');
            await expect(toggleBridgeDirectionButton).toBeVisible();
            await toggleBridgeDirectionButton.click();

            // Manualy input addressL1 on pageWithL2Wallet
            const toggleManualInputL2 = pageWithL2Wallet.getByTestId(
                'toggle-receiver-address-input',
            );
            await expect(toggleManualInputL2).toBeVisible();
            await toggleManualInputL2.click();

            const addressFieldL2 = pageWithL2Wallet.getByTestId('receive-address');
            await expect(addressFieldL2).toBeVisible();
            await addressFieldL2.fill(addressL1);

            // Fund L1 wallet with IOTA and native tokens
            await addL1FundsThroughBridgeUI(pageWithL1Wallet, browserL1);
            const nativeTokenAmount = 5;
            await fundL1AddressWithNativeTokens(addressL1, nativeTokenAmount);
            // Fund L2 wallet with IOTA
            await fundL2AddressWithIscClient(addressL2, 5);
        },
    );

    test('should successfully process an L1 deposit', async () => {
        const nativeTokenAmount = 3;

        const l1CoinBalance = await checkL1CoinBalanceForAddressWithRetries(
            addressL1 ?? '',
            TOOL_COIN_TYPE,
        );
        expect(Number(l1CoinBalance)).toBeGreaterThan(nativeTokenAmount);

        await pageWithL1Wallet.getByTestId('coin-selector').click();
        await pageWithL1Wallet.getByText('Tool', { exact: true }).click();

        const amountField = pageWithL1Wallet.getByTestId('bridge-amount');
        await expect(amountField).toBeVisible();
        amountField.fill(nativeTokenAmount.toString());

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

        await expect(pageWithL1Wallet.getByText('Bridge Assets')).toBeEnabled();
        await pageWithL1Wallet.getByText('Bridge Assets').click();

        const approveTransactionPage = await browserL1.waitForEvent('page');
        await approveTransactionPage.waitForLoadState();
        await approveTransactionPage.getByRole('button', { name: 'Approve' }).click();

        const balance = await checkL2CoinBalanceForAddressWithRetries(
            addressL2 ?? '',
            TOOL_COIN_TYPE,
        );
        expect(balance).toEqual(nativeTokenAmount.toString());
    });

    test('should successfully process an L2 deposit', async () => {
        const nativeTokenAmount = '2';

        await pageWithL2Wallet.getByTestId('coin-selector').click();
        await pageWithL2Wallet.getByText('Tool', { exact: true }).click();

        const amountField = pageWithL2Wallet.getByTestId('bridge-amount');
        await expect(amountField).toBeVisible();
        await amountField.fill(nativeTokenAmount);

        // check est. gas fees and your receive
        await pageWithL2Wallet.waitForTimeout(2500);

        const gasFeeValue = await pageWithL2Wallet
            .locator('div:has(> span:text("Est. IOTA EVM Gas Fees"))')
            .locator('xpath=../div/span')
            .nth(1)
            .textContent();
        expect(Number(gasFeeValue).toFixed(6)).toMatch(/^0\.0003\d\d$/);

        await expect(pageWithL2Wallet.getByText('Bridge Assets')).toBeEnabled();

        const approveTransactionPagePromise = browserL2.waitForEvent('page');
        await pageWithL2Wallet.getByText('Bridge Assets').click();

        const approveTransactionPage = await approveTransactionPagePromise;
        await approveTransactionPage.getByRole('button', { name: 'Confirm' }).click();

        // Check funds on L1 wallet
        const l1Balance = await checkL1CoinBalanceForAddressWithRetries(
            addressL1 ?? '',
            TOOL_COIN_TYPE,
        );
        expect(l1Balance).toEqual(nativeTokenAmount.toString());
    });
});
