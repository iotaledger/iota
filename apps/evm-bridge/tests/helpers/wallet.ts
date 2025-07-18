import { BrowserContext, Page } from '@playwright/test';
import { CONFIG } from '../config/config';
import { HDNodeWallet, Wallet } from 'ethers';
import { WALLET_CUSTOMRPC_PLACEHOLDER } from '../utils/constants';

export async function createL1Wallet(page: Page, l1ExtensionUrl: string) {
    await page.goto(l1ExtensionUrl);

    await page.getByRole('button', { name: /Add Profile/ }).click();
    await page.getByText('Create New').click();

    await page.getByTestId('password.input').fill('iotae2etests');
    await page.getByTestId('password.confirmation').fill('iotae2etests');
    await page.getByText('I read and agree').click();
    await page.getByRole('button', { name: /Create Wallet/ }).click();
    await page.getByText('I saved my mnemonic').click();
    await page.getByRole('button', { name: /Open Wallet/ }).click();
    await page.getByLabel(/Open settings menu/).click();
    await page.getByText(/Network/).click();
    await page.getByText(/Custom RPC/).click();
    await page.getByPlaceholder(WALLET_CUSTOMRPC_PLACEHOLDER).fill(CONFIG.L1.rpcUrl);
    await page.getByText(/Save/).click();
    await page.getByTestId('close-icon').click();
}

export async function importL1WalletFromMnemonic(
    page: Page,
    l1ExtensionUrl: string,
    mnemonic: string | string[],
) {
    await page.goto(l1ExtensionUrl, { waitUntil: 'commit' });
    await page.getByRole('button', { name: /Add Profile/ }).click();
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
    await page.getByTestId('password.input').fill('bridgee2etests');
    await page.getByTestId('password.confirmation').fill('bridgee2etests');
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

export async function createL2Wallet(page: Page, l2ExtensionUrl: string): Promise<string> {
    await page.goto(l2ExtensionUrl);

    await page.getByTestId('onboarding-terms-checkbox').click();
    await page.getByRole('button', { name: /Import an existing wallet/ }).click();
    await page.getByRole('button', { name: /No thanks/ }).click();

    const { mnemonic, address } = getRandomL2MnemonicAndAddress();

    const mnemonicWords = mnemonic.split(' ');
    for (let i = 0; i < mnemonicWords.length; i++) {
        await page.getByTestId(`import-srp__srp-word-${i}`).first().fill(mnemonicWords[i]);
    }

    await page.getByRole('button', { name: /Confirm Secret/ }).click();
    await page.getByTestId('create-password-new').fill('iotae2etests');
    await page.getByTestId('create-password-confirm').fill('iotae2etests');
    await page.getByTestId(/create-password-terms/).click();
    await page.getByRole('button', { name: /Import my wallet/ }).click();
    await page.getByRole('button', { name: /Done/ }).click();
    await page.getByRole('button', { name: /Next/ }).click();
    await page.getByRole('button', { name: /Done/ }).click();

    return address;
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
    const approveDialog = browserContext.waitForEvent('page', { timeout: 20_000 });
    await page.getByTestId(/metamask/).click();

    const walletModal = await approveDialog;
    await walletModal.waitForLoadState();
    await walletModal.getByRole('button', { name: 'Connect' }).click();
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

export function getRandomL2MnemonicAndAddress(): { mnemonic: string; address: string } {
    const mnemonic = Wallet.createRandom().mnemonic;

    if (!mnemonic) {
        throw new Error('Failed to generate mnemonic');
    }

    return {
        mnemonic: mnemonic.phrase,
        address: HDNodeWallet.fromMnemonic(mnemonic, `m/44'/60'/0'/0/0`).address,
    };
}
