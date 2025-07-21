import path from 'path';
import { test as base, chromium, Page, type BrowserContext } from '@playwright/test';
import { Ed25519Keypair } from '@iota/iota-sdk/keypairs/ed25519';
import { generate24WordMnemonic, deriveAddressFromMnemonic } from '../utils/utils';
import { closeBrowserTabsExceptLast } from './browser';
import {
    addL1FundsThroughBridgeUI,
    fundL1AddressWithNativeTokens,
    fundL2AddressWithIscClient,
} from './transactions';
import { setReceiverAddress, toggleBridgeDirection } from './ui';
import {
    createL1Wallet,
    connectL1Wallet,
    getRandomL2MnemonicAndAddress,
    createL2Wallet,
    addNetworkToMetaMask,
    connectL2Wallet,
    importL1WalletFromMnemonic,
} from './wallet';

const EXTENSION_L1_PATH = path.join(__dirname, '../../../wallet/dist');
const EXTENSION_L2_PATH = path.join(__dirname, '../../wallet-dist-L2');

const COMMON_ARGS = ['--user-agent=Playwright', '--disable-dev-shm-usage', '--no-sandbox'];

type ExtensionFixtures = {
    contextL1: BrowserContext;
    contextL2: BrowserContext;
    l1ExtensionUrl: string;
    l2ExtensionUrl: string;
};

type L1SetupFixture = {
    browser: BrowserContext;
    page: Page;
    receiverAddress: string;
};

type L2SetupFixture = {
    browser: BrowserContext;
    page: Page;
    senderAddress: string;
    receiverAddress: string;
};

type BridgeSetupFixture = {
    browserL1: BrowserContext;
    browserL2: BrowserContext;
    pageWithL1Wallet: Page;
    pageWithL2Wallet: Page;
    addressL1: string;
    addressL2: string;
};

async function waitForExtension(context: BrowserContext): Promise<string> {
    let [background] = context.serviceWorkers();
    if (!background) {
        background = await context.waitForEvent('serviceworker', { timeout: 30000 });
    }

    await new Promise((resolve) => setTimeout(resolve, 1000));

    const extensionId = background.url().split('/')[2];
    return extensionId;
}

export const baseTest = base.extend<ExtensionFixtures>({
    contextL1: async ({}, use) => {
        const context = await chromium.launchPersistentContext('', {
            headless: false,
            args: [
                ...COMMON_ARGS,
                `--disable-extensions-except=${EXTENSION_L1_PATH}`,
                `--load-extension=${EXTENSION_L1_PATH}`,
            ],
        });

        await use(context);
    },

    contextL2: async ({}, use) => {
        const context = await chromium.launchPersistentContext('', {
            headless: false,
            args: [
                ...COMMON_ARGS,
                `--disable-extensions-except=${EXTENSION_L2_PATH}`,
                `--load-extension=${EXTENSION_L2_PATH}`,
            ],
        });

        await use(context);
    },

    l1ExtensionUrl: async ({ contextL1 }, use) => {
        const extensionId = await waitForExtension(contextL1);
        const extensionUrl = `chrome-extension://${extensionId}/ui.html`;
        await use(extensionUrl);
    },

    l2ExtensionUrl: async ({ contextL2 }, use) => {
        const extensionId = await waitForExtension(contextL2);
        const extensionUrl = `chrome-extension://${extensionId}/home.html`;
        await use(extensionUrl);
    },
});

export const test = baseTest.extend<{
    l1Setup: L1SetupFixture;
    l2Setup: L2SetupFixture;
    roundtripSetup: BridgeSetupFixture;
}>({
    // Setup for L1 tests only
    l1Setup: async ({ contextL1, l1ExtensionUrl }, use) => {
        let testPageL1 = await contextL1.newPage();
        await createL1Wallet(testPageL1, l1ExtensionUrl);

        testPageL1 = await contextL1.newPage();
        await closeBrowserTabsExceptLast(contextL1);
        await testPageL1.goto('/');
        await connectL1Wallet(testPageL1, contextL1);

        const { address: receiverAddress } = getRandomL2MnemonicAndAddress();
        await setReceiverAddress(testPageL1, receiverAddress);

        // Provide the setup to the test
        await use({
            browser: contextL1,
            page: testPageL1,
            receiverAddress,
        });
    },

    // Setup for L2 tests only
    l2Setup: async ({ contextL2, l2ExtensionUrl }, use) => {
        let testPageL2 = await contextL2.newPage();
        const senderAddress = await createL2Wallet(testPageL2, l2ExtensionUrl);
        await addNetworkToMetaMask(testPageL2);

        testPageL2 = await contextL2.newPage();
        await closeBrowserTabsExceptLast(contextL2);
        await testPageL2.goto('/');
        await connectL2Wallet(testPageL2, contextL2);
        await toggleBridgeDirection(testPageL2);

        const keypair = new Ed25519Keypair();
        const receiverAddress = keypair.toIotaAddress();
        await setReceiverAddress(testPageL2, receiverAddress);

        await use({
            browser: contextL2,
            page: testPageL2,
            senderAddress,
            receiverAddress,
        });
    },

    // Full roundtrip setup with both L1 and L2
    roundtripSetup: async ({ contextL1, l1ExtensionUrl, contextL2, l2ExtensionUrl }, use) => {
        // Create L1 wallet
        const mnemonicL1 = generate24WordMnemonic();
        const pageWithL1Wallet = await contextL1.newPage();
        await importL1WalletFromMnemonic(pageWithL1Wallet, l1ExtensionUrl, mnemonicL1);
        const addressL1 = deriveAddressFromMnemonic(mnemonicL1);

        // Create L2 wallet
        const pageWithL2Wallet = await contextL2.newPage();
        const addressL2 = await createL2Wallet(pageWithL2Wallet, l2ExtensionUrl);
        await addNetworkToMetaMask(pageWithL2Wallet);

        // Set up pages
        await pageWithL1Wallet.goto('/');
        await pageWithL2Wallet.goto('/');
        await closeBrowserTabsExceptLast(contextL1);
        await closeBrowserTabsExceptLast(contextL2);

        // Connect wallets and configure
        await connectL1Wallet(pageWithL1Wallet, contextL1);
        await setReceiverAddress(pageWithL1Wallet, addressL2);
        await connectL2Wallet(pageWithL2Wallet, contextL2);
        await toggleBridgeDirection(pageWithL2Wallet);
        await setReceiverAddress(pageWithL2Wallet, addressL1);

        // Fund wallets
        await addL1FundsThroughBridgeUI(pageWithL1Wallet, contextL1);
        await fundL1AddressWithNativeTokens(addressL1, 5);
        await fundL2AddressWithIscClient(addressL2, 5);

        await use({
            browserL1: contextL1,
            browserL2: contextL2,
            pageWithL1Wallet,
            pageWithL2Wallet,
            addressL1,
            addressL2,
        });
    },
});

export const expect = test.expect;
