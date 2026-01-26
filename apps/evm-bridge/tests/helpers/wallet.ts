import { BrowserContext, expect, Page } from '@playwright/test';
import { CONFIG } from '../config/config';
import { WALLET_CUSTOMRPC_PLACEHOLDER, WALLET_PASSWORD } from '../utils/constants';
import { createPage } from './browser';

export async function importL1WalletFromMnemonic(
    page: Page,
    l1ExtensionUrl: string,
    mnemonic: string | string[],
) {
    await page.goto(l1ExtensionUrl, { waitUntil: 'commit' });
    await page.getByRole('button', { name: /Get Started/ }).click({ timeout: 30_000 });
    await page.getByText('Add existing wallet').click();
    await page.getByText('Mnemonic', { exact: true }).click();

    const mnemonicArray = typeof mnemonic === 'string' ? mnemonic.split(' ') : mnemonic;

    if (mnemonicArray.length === 12) {
        await page.locator('button:has(div:has-text("24 words"))').click();
        await page.getByText('12 words').click();
    }
    const wordInputs = page.locator('input[placeholder="Word"]');
    const inputCount = await wordInputs.count();

    for (let i = 0; i < inputCount; i++) {
        await wordInputs.nth(i).fill(mnemonicArray[i]);
    }

    await page.getByText('Add profile').click();
    await page.getByTestId('password.input').fill(WALLET_PASSWORD);
    await page.getByTestId('password.confirmation').fill(WALLET_PASSWORD);
    await page.getByText('I read and agree').click();
    await page.getByRole('button', { name: /Create Wallet/ }).click();

    await page.waitForURL(new RegExp(/^(?!.*protect-account).*$/));

    if (await page.getByText('Balance Finder').isVisible()) {
        await page.getByRole('button', { name: /Skip/ }).click();
    }

    // We need to switch the network to ALPHANET (custom RPC) before requesting
    await page.getByLabel(/Open settings menu/).click();
    await page.getByText(/Network/).click();
    await page.getByText(/Custom RPC/).click();
    await page.getByPlaceholder(WALLET_CUSTOMRPC_PLACEHOLDER).fill(CONFIG.L1.rpcUrl);
    await page.getByText(/Save/).click();
    await page.getByTestId('close-icon').click();
}

export async function createL2Wallet(page: Page, l2ExtensionUrl: string, mnemonic: string) {
    await page.goto(l2ExtensionUrl);

    await page.getByTestId('onboarding-terms-checkbox').click();
    await page.getByRole('button', { name: /Import an existing wallet/ }).click();
    await page.getByRole('button', { name: /No thanks/ }).click();

    const mnemonicWords = mnemonic.split(' ');
    for (let i = 0; i < mnemonicWords.length; i++) {
        await page.getByTestId(`import-srp__srp-word-${i}`).first().fill(mnemonicWords[i]);
    }

    await page.getByRole('button', { name: /Confirm Secret/ }).click();
    await page.getByTestId('create-password-new').fill(WALLET_PASSWORD);
    await page.getByTestId('create-password-confirm').fill(WALLET_PASSWORD);
    await page.getByTestId(/create-password-terms/).click();
    await page.getByRole('button', { name: /Import my wallet/ }).click();
    await page.getByRole('button', { name: /Done/ }).click();
    await page.getByRole('button', { name: /Next/ }).click();
    await page.getByRole('button', { name: /Done/ }).click();
}

/**
 * Connect L1 wallet to the bridge UI
 */
export async function connectL1Wallet(page: Page, browserContext: BrowserContext): Promise<void> {
    const connectButtonId = 'connect-l1-wallet';
    const connectButton = await page.waitForSelector(`[data-testid="${connectButtonId}"]`, {
        state: 'visible',
    });

    await connectButton.click();
    const approveWalletConnectPage = browserContext.waitForEvent('page');
    await page.getByText('IOTA Wallet').click();

    const walletPage = await approveWalletConnectPage;
    await walletPage.waitForLoadState();
    await walletPage.getByRole('button', { name: 'Continue' }).click();
    await walletPage.getByRole('button', { name: 'Connect' }).click();
}

/**
 * Connect L2 wallet to the bridge UI
 */
export async function connectL2Wallet(page: Page, browserContext: BrowserContext): Promise<void> {
    const connectButtonId = 'connect-l2-wallet';
    const connectButton = await page.waitForSelector(`[data-testid="${connectButtonId}"]`, {
        state: 'visible',
    });

    await connectButton.click();

    const metamaskButton = page.getByRole('button', { name: 'MetaMask', exact: true });
    await metamaskButton.waitFor({ state: 'visible', timeout: 10000 });

    const approveDialogPromise = waitForMetaMaskDialog(browserContext, page, 30000);
    await metamaskButton.click();

    const walletModal = await approveDialogPromise;
    await walletModal.waitForLoadState();

    await walletModal.getByRole('button', { name: 'Connect' }).click();
}

async function waitForMetaMaskDialog(
    browserContext: BrowserContext,
    mainPage: Page,
    timeout = 20000,
): Promise<Page> {
    try {
        return await browserContext.waitForEvent('page', { timeout });
    } catch (error) {
        const allPages = browserContext.pages();
        const potentialDialogs = allPages.filter(
            (p) =>
                p !== mainPage &&
                p.url().includes('notification') &&
                p.url().includes('chrome-extension'),
        );

        if (potentialDialogs.length > 0) {
            return potentialDialogs[0];
        }

        throw error;
    }
}

export async function addNetworkToMetaMask(l2WalletPage: Page) {
    await l2WalletPage.click('[data-testid="network-display"]', { force: true });
    const popoverCloseButton = l2WalletPage.locator('.page-container__header-close');

    if (await popoverCloseButton.isVisible()) {
        await popoverCloseButton.click();
    }
    const addCustomNetworkButton = await l2WalletPage.getByText('Add a custom network');

    if (await addCustomNetworkButton.isHidden()) {
        await l2WalletPage.click('[data-testid="network-display"]');
    }

    await addCustomNetworkButton.click();

    await l2WalletPage.getByTestId('network-form-network-name').fill(CONFIG.L2.chainName);
    await l2WalletPage.getByTestId('test-add-rpc-drop-down').click();
    await l2WalletPage.getByText('Add RPC URL').click();
    await l2WalletPage.getByTestId('rpc-url-input-test').fill(CONFIG.L2.rpcUrl);
    await l2WalletPage.getByText('Add URL').click();

    await l2WalletPage.getByTestId('network-form-chain-id').fill(CONFIG.L2.chainId.toString());
    await l2WalletPage.getByTestId('network-form-ticker-input').fill(CONFIG.L2.chainCurrency);

    await l2WalletPage.getByText('Save').click();

    await l2WalletPage.click('[data-testid="network-display"]');
    await l2WalletPage.getByRole('button', { name: CONFIG.L2.chainName }).click();
}

/**
 * Set up L1 wallet with mnemonic
 */
export async function setupL1Wallet(
    context: BrowserContext,
    l1ExtensionUrl: string,
    mnemonic: string,
    testId?: string,
): Promise<void> {
    const walletPageL1 = await createPage(context, l1ExtensionUrl);
    try {
        await importL1WalletFromMnemonic(walletPageL1, l1ExtensionUrl, mnemonic);
    } catch (error) {
        console.error('Error setting up L1 wallet:', error);
        throw error;
    } finally {
        await walletPageL1.close().catch((e) => console.error('Error closing L1 wallet page:', e));
    }
    console.log('✅ L1 wallet setup complete', testId);
}

/**
 * Set up L2 wallet with mnemonic
 */
export async function setupL2Wallet(
    context: BrowserContext,
    l2ExtensionUrl: string,
    mnemonic: string,
    testId?: string,
): Promise<void> {
    const walletPageL2 = await createPage(context, l2ExtensionUrl);
    try {
        await createL2Wallet(walletPageL2, l2ExtensionUrl, mnemonic);
        await addNetworkToMetaMask(walletPageL2);
    } catch (error) {
        console.error('Error setting up L2 wallet:', error);
        throw error;
    } finally {
        await walletPageL2.close().catch((e) => console.error('Error closing L2 wallet page:', e));
    }
    console.log('✅ L2 wallet setup complete', testId);
}

/**
 * Set up both wallets for bridge testing
 */
export async function setupBridgeWallets(
    context: BrowserContext,
    l1ExtensionUrl: string,
    l2ExtensionUrl: string,
    mnemonicL1: string,
    mnemonicL2: string,
    testId?: string,
): Promise<void> {
    await setupL1Wallet(context, l1ExtensionUrl, mnemonicL1, testId);
    await setupL2Wallet(context, l2ExtensionUrl, mnemonicL2, testId);
}

export async function waitForL1WalletConnected(
    page: Page,
    { timeout = 30000 } = {},
    testId?: string,
): Promise<void> {
    try {
        const senderAddressInput = page.locator('input[name="senderAddress"]');
        await senderAddressInput.waitFor({
            state: 'visible',
            timeout,
        });

        await expect(senderAddressInput).toBeEnabled();

        console.log(`✅ L1 wallet connected (senderAddress input is visible): ${testId}`);
    } catch (error) {
        console.error('❌ L1 wallet connection failed:', error, testId);
        throw new Error(`L1 wallet connection check failed: ${error.message}, testId: ${testId}`);
    }
}
