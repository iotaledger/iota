import { Page } from '@playwright/test';
import { CONFIG } from '../config/config';
import { getRandomL2MnemonicAndAddress } from './utils';

const WALLET_CUSTOMRPC_PLACEHOLDER = 'http://localhost:3000/';

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
    await page.getByTestId('onboarding-get-started-button').click();

    await page.getByTestId('terms-of-use-checkbox').click();

    await page.locator('.mm-box.terms-of-use-popup__body').focus();
    // Press End key multiple times to ensure we reach the bottom
    await page.keyboard.press('End');

    // Wait for button to be enabled
    await page.getByTestId('terms-of-use-agree-button').isEnabled({ timeout: 10000 });
    await page.getByTestId('terms-of-use-agree-button').click();

    await page.getByTestId('onboarding-import-wallet').click();

    const { mnemonic, address } = getRandomL2MnemonicAndAddress();
    const mnemonicWords = mnemonic.split(' ');

    await page.getByTestId('srp-input-import__srp-note').focus();

    // Type each word manually with spaces in between
    for (let i = 0; i < mnemonicWords.length; i++) {
        // Type the word
        await page.keyboard.type(mnemonicWords[i]);

        // Add space after each word except the last one
        if (i < mnemonicWords.length - 1) {
            await page.keyboard.press('Space');
        }

        // Optional: small delay between words to make it look more human-like
        await page.waitForTimeout(50);
    }

    await page.getByTestId('import-srp-confirm').isEnabled({ timeout: 10000 });
    await page.getByTestId('import-srp-confirm').click();

    await page.getByTestId('create-password-new-input').fill('iotae2etests');
    await page.getByTestId('create-password-confirm-input').fill('iotae2etests');
    await page.getByTestId(/create-password-terms/).click();
    await page.getByTestId('create-password-submit').click();
    await page.getByTestId('metametrics-no-thanks').click();

    await page.getByRole('button', { name: /Done/ }).click();
    await page.getByRole('button', { name: /Done/ }).click();

    return address;
}
