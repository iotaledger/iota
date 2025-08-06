import { ethers, Wallet, HDNodeWallet, JsonRpcProvider } from 'ethers';
import { BrowserContext, Page } from '@playwright/test';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import {
    AccountsContractMethod,
    CoreContract,
    getHname,
    IscTransaction,
    L2_FROM_L1_GAS_BUDGET,
} from '@iota/isc-sdk';
import { IotaClient } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { requestIotaFromFaucetV0 } from '@iota/iota-sdk/faucet';
import { CONFIG } from '../config/config';
import { expect } from './fixtures';

const THREE_MINUTES = 180_000;

export function generate24WordMnemonic() {
    const entropy = ethers.randomBytes(32);
    return ethers.Mnemonic.fromEntropy(entropy).phrase;
}

export function deriveAddressFromMnemonic(mnemonic: string) {
    const keypair = Ed25519Keypair.deriveKeypair(mnemonic);
    const address = keypair.getPublicKey().toIotaAddress();
    return address;
}

async function checkBalanceWithRetries(
    address: string,
    fetchBalance: (address: string) => Promise<string | null>,
    layer: 'L1' | 'L2',
    maxRetries = 10,
    delay = 2500,
): Promise<string | null> {
    let balance: string | null = null;

    for (let attempt = 1; attempt <= maxRetries; attempt++) {
        try {
            balance = await fetchBalance(address);
        } catch (error) {
            console.error('Error checking balance:', error);
        } finally {
            if (balance?.startsWith('0') && attempt < maxRetries) {
                console.log(
                    `Fetching ${layer} balance attempt ${attempt + 1} out of ${maxRetries} in ${delay} ms`,
                );
                await new Promise((resolve) => setTimeout(resolve, delay));
            }
        }
    }

    return balance;
}

export async function getL1BalanceForAddress(address: string): Promise<string> {
    const { L1 } = CONFIG;

    const client = new IotaClient({
        url: L1.rpcUrl,
    });

    const balance = await client.getBalance({ owner: address });

    return ethers.formatUnits(balance.totalBalance, 9);
}

export async function getEVMBalanceForAddress(address: string): Promise<string> {
    const provider = new JsonRpcProvider(CONFIG.L2.rpcUrl);
    const balanceWei = await provider.getBalance(address);

    return ethers.formatEther(balanceWei);
}

export async function checkL1BalanceWithRetries(address: string) {
    return await checkBalanceWithRetries(address, getL1BalanceForAddress, 'L1');
}

export async function checkL2BalanceWithRetries(address: string) {
    return await checkBalanceWithRetries(address, getEVMBalanceForAddress, 'L2');
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

export async function fundL2AddressWithIscClient(addressL2: string, amount: number) {
    const { L1 } = CONFIG;

    const client = new IotaClient({
        url: L1.rpcUrl,
    });

    const keypair = new Ed25519Keypair();
    const address = keypair.toIotaAddress();

    await requestIotaFromFaucetV0({
        host: L1.faucetUrl!,
        recipient: address,
    });

    const amountToSend = BigInt(amount * 1000000000);
    const amountToPlace = amountToSend + L2_FROM_L1_GAS_BUDGET;

    const iscTx = new IscTransaction(L1);

    const bag = iscTx.newBag();
    const coin = iscTx.coinFromAmount({ amount: amountToPlace });
    iscTx.placeCoinInBag({ coin, bag });
    iscTx.createAndSendToEvm({
        bag,
        transfers: [[IOTA_TYPE_ARG, amountToSend]],
        address: addressL2,
        accountsContract: getHname(CoreContract.Accounts),
        accountsFunction: getHname(AccountsContractMethod.TransferAllowanceTo),
    });

    const transaction = iscTx.build();
    transaction.setSender(address);
    await transaction.build({ client });

    await client.signAndExecuteTransaction({
        signer: keypair,
        transaction,
    });
}

// Playwright
export async function closeBrowserTabsExceptLast(browserContext: BrowserContext) {
    const pages = browserContext.pages();
    if (pages.length > 1) {
        for (let i = 0; i < pages.length - 1; i++) {
            await pages[i].close();
        }
    }
}

export async function getExtensionUrl(browserContext: BrowserContext): Promise<string> {
    let [background] = browserContext.serviceWorkers();

    if (!background) {
        background = await browserContext.waitForEvent('serviceworker', { timeout: 30000 });
    }

    const extensionId = background.url().split('/')[2];
    return `chrome-extension://${extensionId}/ui.html`;
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

export async function addL1FundsThroughBridgeUI(page: Page, browser: BrowserContext) {
    const maxRetries = 3; // Maximum number of retry attempts
    let attempt = 1;
    let success = false;

    while (attempt <= maxRetries && !success) {
        try {
            console.log(`Attempt ${attempt}/${maxRetries} to add funds through bridge UI`);

            // Add funds to L1
            await page.getByTestId('request-l1-funds-button').click();

            // Wait for transaction completion - look for either success or error message
            const successPromise = page
                .getByText('Funds successfully sent.')
                .waitFor({ timeout: 30000 })
                .then(() => 'success')
                .catch(() => 'timeout');

            const errorPromise = page
                .getByText('Something went wrong while requesting funds.')
                .waitFor({ timeout: 30000 })
                .then(() => 'error')
                .catch(() => 'timeout');

            // Wait for either message to appear
            const result = await Promise.race([successPromise, errorPromise]);

            if (result === 'success') {
                console.log('✅ Bridge funding transaction successful: Funds sent from faucet!');
                success = true;
            } else if (result === 'error') {
                console.log(
                    `❌ Bridge funding transaction failed on attempt ${attempt}/${maxRetries}, retrying...`,
                );
                await page.pause();
                // Wait a bit before retrying
                await page.waitForTimeout(3000);
            } else {
                console.log(
                    '⏱️ Bridge funding transaction timed out on attempt ${attempt}/${maxRetries}, retrying...',
                );
                await page.waitForTimeout(3000);
            }
        } catch (error) {
            console.error(`Error during attempt ${attempt}:`, error);
        }

        attempt++;
    }

    if (!success) {
        throw new Error(`Failed to add funds trough bridge UI after ${maxRetries} attempts`);
    }

    // Check the funds arrived (ui)
    const l1WalletExtension = await browser.newPage();
    const l1ExtensionUrl = await getExtensionUrl(browser);
    await l1WalletExtension.goto(l1ExtensionUrl, { waitUntil: 'commit' });
    await expect(l1WalletExtension.getByTestId('coin-balance')).toHaveText('10', {
        timeout: THREE_MINUTES,
    });
    await l1WalletExtension.close();
}
