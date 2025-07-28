/* eslint-disable no-empty-pattern */
import path from 'path';
import { test as base, chromium, Page, type BrowserContext } from '@playwright/test';
import { waitForExtension, waitForExtensions } from './browser';
import {
    connectL1Wallet,
    createL2Wallet,
    addNetworkToMetaMask,
    connectL2Wallet,
    importL1WalletFromMnemonic,
} from './wallet';
import { getSharedState } from './shared-state';
import { setReceiverAddress, toggleBridgeDirection } from './ui';
import { TestWalletData } from '../utils/utils';

const EXTENSION_L1_PATH = path.join(__dirname, '../../../wallet/dist');
const EXTENSION_L2_PATH = path.join(__dirname, '../../wallet-dist-L2');

const COMMON_ARGS = ['--user-agent=Playwright', '--disable-dev-shm-usage', '--no-sandbox'];

type ExtensionFixtures = {
    contextL1: BrowserContext;
    contextL2: BrowserContext;
    roundtripIotaContext: BrowserContext;
    roundtripNativeTokenContext: BrowserContext;
    l1ExtensionUrl: string;
    l2ExtensionUrl: string;
    roundtripIotaExtensions: { l1ExtensionUrl: string; l2ExtensionUrl: string };
    roundtripNativeTokenExtensions: { l1ExtensionUrl: string; l2ExtensionUrl: string };
};

type BridgeSetupFixture = {
    browser: BrowserContext;
    page: Page;
    addressL1: string;
    addressL2: string;
};

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
        await context.close();
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
        await context.close();
    },
    // Add named persistent contexts
    roundtripIotaContext: async ({}, use) => {
        console.log('🔄 Creating persistent context for IOTA deposit tests');
        const context = await chromium.launchPersistentContext('', {
            headless: false,
            args: [
                ...COMMON_ARGS,
                `--disable-extensions-except=${EXTENSION_L1_PATH},${EXTENSION_L2_PATH}`,
                `--load-extension=${EXTENSION_L1_PATH},${EXTENSION_L2_PATH}`,
            ],
        });

        await use(context);
        // No context.close() here - will be closed by test
    },

    roundtripNativeTokenContext: async ({}, use) => {
        console.log('🔄 Creating persistent context for native token deposit tests');
        const context = await chromium.launchPersistentContext('', {
            headless: false,
            args: [
                ...COMMON_ARGS,
                `--disable-extensions-except=${EXTENSION_L1_PATH},${EXTENSION_L2_PATH}`,
                `--load-extension=${EXTENSION_L1_PATH},${EXTENSION_L2_PATH}`,
            ],
        });

        await use(context);
        // No context.close() here - will be closed by test
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
    roundtripIotaExtensions: async ({ roundtripIotaContext }, use) => {
        const extensions = await waitForExtensions(roundtripIotaContext);
        await use(extensions);
    },
    roundtripNativeTokenExtensions: async ({ roundtripNativeTokenContext }, use) => {
        const extensions = await waitForExtensions(roundtripNativeTokenContext);
        await use(extensions);
    },
});

// Create a generic setup fixture
export const test = baseTest.extend<{
    roundtripIotaSetup: (testId: string) => Promise<BridgeSetupFixture>;
    roundtripNativeTokenSetup: (testId: string) => Promise<BridgeSetupFixture>;
    browserWithL1Setup: (testId: string) => Promise<BridgeSetupFixture>;
    browserWithL2Setup: (testId: string) => Promise<BridgeSetupFixture>;
}>({
    roundtripIotaSetup: async ({ roundtripIotaContext, roundtripIotaExtensions }, use) => {
        // This function will be provided to each test
        const setupFn = async (testId: string): Promise<BridgeSetupFixture> => {
            console.log('Setting up browser for test:', testId);

            const state = getSharedState();
            const testData: TestWalletData = state.tests[testId];
            if (!testData) throw new Error(`No test data found for ID: ${testId}`);
            const { addressL1, addressL2, mnemonicL1, mnemonicL2 } = testData;

            const { l1ExtensionUrl, l2ExtensionUrl } = roundtripIotaExtensions;
            console.log('Setting up browser l1ExtensionUrl:', l1ExtensionUrl);
            console.log('Setting up browser l2ExtensionUrl:', l2ExtensionUrl);
            // Import/unlock wallets if mnemonics are provided
            console.log('Importing L1 wallet from mnemonic');
            const walletPageL1 = await roundtripIotaContext.newPage();
            await walletPageL1.waitForTimeout(500); // Wait for the app to load
            // await closeBrowserTabsExceptLast(roundtripIotaContext);
            await walletPageL1.goto(l1ExtensionUrl);
            await walletPageL1.bringToFront();
            await importL1WalletFromMnemonic(walletPageL1, l1ExtensionUrl, mnemonicL1);
            await walletPageL1.close();

            console.log('Creating L2 wallet from mnemonic');
            const walletPageL2 = await roundtripIotaContext.newPage();
            await walletPageL2.waitForTimeout(500);
            // await closeBrowserTabsExceptLast(roundtripIotaContext);
            await walletPageL2.goto(l2ExtensionUrl);
            await walletPageL2.bringToFront();
            await createL2Wallet(walletPageL2, l2ExtensionUrl, mnemonicL2);
            await addNetworkToMetaMask(walletPageL2);
            await walletPageL2.close();

            // Create page for evm bridge tests
            const page = await roundtripIotaContext.newPage();
            await page.waitForTimeout(500);
            // await closeBrowserTabsExceptLast(roundtripIotaContext);
            await page.goto('/');
            await page.bringToFront();
            // Set up wallet connections
            await page.waitForTimeout(500); // Wait for the app to load
            await connectL1Wallet(page, roundtripIotaContext);

            await page.waitForTimeout(500);
            await connectL2Wallet(page, roundtripIotaContext);

            return {
                browser: roundtripIotaContext,
                page,
                addressL1,
                addressL2,
            };
        };

        // Provide the setup function to the test
        await use(setupFn);
    },
    roundtripNativeTokenSetup: async (
        { roundtripNativeTokenContext, roundtripNativeTokenExtensions },
        use,
    ) => {
        // This function will be provided to each test
        const setupFn = async (testId: string): Promise<BridgeSetupFixture> => {
            console.log('Setting up browser for test:', testId);

            const state = getSharedState();
            const testData: TestWalletData = state.tests[testId];
            if (!testData) throw new Error(`No test data found for ID: ${testId}`);
            const { addressL1, addressL2, mnemonicL1, mnemonicL2 } = testData;

            const { l1ExtensionUrl, l2ExtensionUrl } = roundtripNativeTokenExtensions;
            console.log('Setting up browser l1ExtensionUrl:', l1ExtensionUrl);
            console.log('Setting up browser l2ExtensionUrl:', l2ExtensionUrl);
            // Import/unlock wallets if mnemonics are provided
            console.log('Importing L1 wallet from mnemonic');
            const walletPageL1 = await roundtripNativeTokenContext.newPage();
            await walletPageL1.waitForTimeout(500); // Wait for the app to load
            // await closeBrowserTabsExceptLast(roundtripNativeTokenContext);
            await walletPageL1.goto(l1ExtensionUrl);
            await walletPageL1.bringToFront();
            await importL1WalletFromMnemonic(walletPageL1, l1ExtensionUrl, mnemonicL1);
            await walletPageL1.close();

            console.log('Creating L2 wallet from mnemonic');
            const walletPageL2 = await roundtripNativeTokenContext.newPage();
            await walletPageL2.waitForTimeout(500); // Wait for the app to load
            // await closeBrowserTabsExceptLast(roundtripNativeTokenContext);
            await walletPageL2.goto(l2ExtensionUrl);
            await walletPageL2.bringToFront();
            await createL2Wallet(walletPageL2, l2ExtensionUrl, mnemonicL2);
            await addNetworkToMetaMask(walletPageL2);
            await walletPageL2.close();

            // Create page for evm bridge tests
            const page = await roundtripNativeTokenContext.newPage();
            await page.waitForTimeout(500); // Wait for the app to load
            // await closeBrowserTabsExceptLast(roundtripNativeTokenContext);
            await page.goto('/');
            await page.bringToFront();
            // await closeBrowserTabsExceptLast(roundtripNativeTokenContext);
            // Set up wallet connections
            await page.waitForTimeout(500); // Wait for the app to load
            await connectL1Wallet(page, roundtripNativeTokenContext);

            await page.waitForTimeout(500);
            await connectL2Wallet(page, roundtripNativeTokenContext);

            return {
                browser: roundtripNativeTokenContext,
                page,
                addressL1,
                addressL2,
            };
        };

        // Provide the setup function to the test
        await use(setupFn);
    },
    // L1-only setup (IOTA Wallet)
    browserWithL1Setup: async ({ contextL1, l1ExtensionUrl }, use) => {
        // This function will be provided to each test
        const setupFn = async (testId: string): Promise<BridgeSetupFixture> => {
            console.log('Setting up L1 browser for test:', testId);
            const state = getSharedState();
            const testData: TestWalletData = state.tests[testId];
            if (!testData) throw new Error(`No test data found for ID: ${testId}`);
            const { addressL1, mnemonicL1, addressL2 } = testData;

            console.log('Setting up L1 browser with extension URL:', l1ExtensionUrl);

            // Import/unlock L1 wallet if mnemonic is provided
            console.log('Importing L1 wallet from mnemonic');
            const walletPageL1 = await contextL1.newPage();
            await walletPageL1.goto(l1ExtensionUrl);
            await importL1WalletFromMnemonic(walletPageL1, l1ExtensionUrl, mnemonicL1);
            await walletPageL1.close();

            // Create page for evm bridge tests
            const page = await contextL1.newPage();
            // await page.waitForTimeout(2500); // Wait for the app to load
            await page.goto('/');

            // Set up wallet connection
            await connectL1Wallet(page, contextL1);

            await setReceiverAddress(page, addressL2);

            return {
                browser: contextL1,
                page,
                addressL1,
                addressL2,
            };
        };

        // Provide the setup function to the test
        await use(setupFn);
    },
    // L2-only setup (MetaMask)
    browserWithL2Setup: async ({ contextL2, l2ExtensionUrl }, use) => {
        // This function will be provided to each test
        const setupFn = async (testId: string): Promise<BridgeSetupFixture> => {
            console.log('Setting up L2 browser for test:', testId);
            const state = getSharedState();
            const testData: TestWalletData = state.tests[testId];
            if (!testData) throw new Error(`No test data found for ID: ${testId}`);
            const { addressL2, mnemonicL2, addressL1 } = testData;

            console.log('Setting up L2 browser with extension URL:', l2ExtensionUrl);

            // Import/unlock L2 wallet if mnemonic is provided
            console.log('Creating L2 wallet from mnemonic');
            const walletPageL2 = await contextL2.newPage();
            await walletPageL2.goto(l2ExtensionUrl);
            await createL2Wallet(walletPageL2, l2ExtensionUrl, mnemonicL2);
            await addNetworkToMetaMask(walletPageL2);
            await walletPageL2.close();

            // Create page for evm bridge tests
            const page = await contextL2.newPage();
            // await page.waitForTimeout(500); // Wait for the app to load
            await page.goto('/');

            // Set up wallet connection for L2
            await connectL2Wallet(page, contextL2);
            await toggleBridgeDirection(page);
            await setReceiverAddress(page, addressL1);

            return {
                browser: contextL2,
                page,
                addressL1,
                addressL2,
            };
        };

        // Provide the setup function to the test
        await use(setupFn);
    },
});

export const expect = test.expect;
