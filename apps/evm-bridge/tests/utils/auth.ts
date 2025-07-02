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
