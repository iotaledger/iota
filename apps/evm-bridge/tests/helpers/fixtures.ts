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

const CONTEXT_CONFIGS = {
    l1Only: {
        args: [
            ...COMMON_ARGS,
            `--disable-extensions-except=${EXTENSION_L1_PATH}`,
            `--load-extension=${EXTENSION_L1_PATH}`,
        ],
    },
    l2Only: {
        args: [
            ...COMMON_ARGS,
            `--disable-extensions-except=${EXTENSION_L2_PATH}`,
            `--load-extension=${EXTENSION_L2_PATH}`,
        ],
    },
    both: {
        args: [
            ...COMMON_ARGS,
            `--disable-extensions-except=${EXTENSION_L1_PATH},${EXTENSION_L2_PATH}`,
            `--load-extension=${EXTENSION_L1_PATH},${EXTENSION_L2_PATH}`,
        ],
    },
};

type BridgeSetupFixture = {
    browser: BrowserContext;
    page: Page;
    addressL1: string;
    addressL2: string;
};

interface ContextFactoryOptions {
    persistent?: boolean;
    name?: string;
    extensions?: 'l1' | 'l2' | 'both';
}
const nonPersistentContexts = new Set<BrowserContext>();

export const baseTest = base.extend<{
    createContext: (options?: ContextFactoryOptions) => Promise<BrowserContext>;
}>({
    // Context factory that creates customized browser contexts
    createContext: async ({}, use) => {
        // Factory function that returns a new context each time it's called
        const contextFactory = async (options: ContextFactoryOptions = {}) => {
            const {
                persistent = false,
                name = 'context',
                extensions = 'both', // 'l1', 'l2', or 'both'
            } = options;

            console.log(
                `🔄 Creating ${persistent ? 'persistent' : 'auto-closing'} context: ${name}`,
            );

            // Determine which extensions to load
            let extensionArgs: string[] = [];
            if (extensions === 'l1') {
                extensionArgs = CONTEXT_CONFIGS.l1Only.args;
            } else if (extensions === 'l2') {
                extensionArgs = CONTEXT_CONFIGS.l2Only.args;
            } else {
                extensionArgs = CONTEXT_CONFIGS.both.args;
            }

            // Create the context
            const context = await chromium.launchPersistentContext('', {
                headless: false,
                args: [...COMMON_ARGS, ...extensionArgs],
            });

            // If not persistent, register a finalizer to close the context when test is don
            if (!persistent) {
                nonPersistentContexts.add(context);
            }
            return context;
        };

        await use(contextFactory);
        for (const context of nonPersistentContexts) {
            try {
                if (!context.browser()?.isConnected()) continue; // Already closed
                await context.close().catch((e) => console.error('Error closing context:', e));
            } catch (e) {
                // Ignore errors during cleanup
            }
        }
        nonPersistentContexts.clear();
    },
});

// Create a generic setup fixture
export const test = baseTest.extend<{
    browserWithBothExtensionsSetup: (testId: string) => Promise<BridgeSetupFixture>;
    browserWithL1Setup: (testId: string) => Promise<BridgeSetupFixture>;
    browserWithL2Setup: (testId: string) => Promise<BridgeSetupFixture>;
}>({
    // Both L1 and L2 setup
    browserWithBothExtensionsSetup: async ({ createContext }, use) => {
        const setupFn = async (testId: string): Promise<BridgeSetupFixture> => {
            console.log('Setting up browser for test:', testId);
            const context = await createContext({
                persistent: true,
                name: `${testId}-context`,
                extensions: 'both',
            });

            const { l1ExtensionUrl, l2ExtensionUrl } = await waitForExtensions(context);
            console.log('Setting up browser l1ExtensionUrl:', l1ExtensionUrl);
            console.log('Setting up browser l2ExtensionUrl:', l2ExtensionUrl);

            const state = getSharedState();
            const testData: TestWalletData = state.tests[testId];
            if (!testData) throw new Error(`No test data found for ID: ${testId}`);
            const { addressL1, addressL2, mnemonicL1, mnemonicL2 } = testData;

            // Import/unlock wallets if mnemonics are provided
            console.log('Importing L1 wallet from mnemonic');
            const walletPageL1 = await context.newPage();
            await walletPageL1.waitForTimeout(1500); // Wait for the app to load
            // await closeBrowserTabsExceptLast(roundtripIotaContext);
            await walletPageL1.goto(l1ExtensionUrl);
            await walletPageL1.bringToFront();
            await importL1WalletFromMnemonic(walletPageL1, l1ExtensionUrl, mnemonicL1);
            await walletPageL1.close();

            console.log('Creating L2 wallet from mnemonic');
            const walletPageL2 = await context.newPage();
            await walletPageL2.waitForTimeout(1500);
            // await closeBrowserTabsExceptLast(roundtripIotaContext);
            await walletPageL2.goto(l2ExtensionUrl);
            await walletPageL2.bringToFront();
            await createL2Wallet(walletPageL2, l2ExtensionUrl, mnemonicL2);
            await addNetworkToMetaMask(walletPageL2);
            await walletPageL2.close();

            // Create page for evm bridge tests
            const page = await context.newPage();
            await page.waitForTimeout(2500);
            // await closeBrowserTabsExceptLast(roundtripIotaContext);
            await page.goto('/');
            await page.bringToFront();
            // Set up wallet connections
            await page.waitForTimeout(500); // Wait for the app to load
            await connectL1Wallet(page, context);

            await page.waitForTimeout(500);
            await connectL2Wallet(page, context);

            return {
                browser: context,
                page,
                addressL1,
                addressL2,
            };
        };

        // Provide the setup function to the test
        await use(setupFn);
    },
    // L1-only setup (IOTA Wallet)
    browserWithL1Setup: async ({ createContext }, use) => {
        // This function will be provided to each test
        const setupFn = async (testId: string): Promise<BridgeSetupFixture> => {
            console.log('Setting up L1 browser for test:', testId);
            const context = await createContext({
                name: `${testId}-context`,
                extensions: 'l1',
            });

            const extensionId = await waitForExtension(context);
            const extensionUrl = `chrome-extension://${extensionId}/ui.html`;

            const state = getSharedState();
            const testData: TestWalletData = state.tests[testId];
            if (!testData) throw new Error(`No test data found for ID: ${testId}`);
            const { addressL1, mnemonicL1, addressL2 } = testData;

            console.log('Setting up L1 browser with extension URL:', extensionUrl);

            // Import/unlock L1 wallet if mnemonic is provided
            console.log('Importing L1 wallet from mnemonic');
            const walletPageL1 = await context.newPage();
            await walletPageL1.waitForTimeout(2500);
            await walletPageL1.goto(extensionUrl);
            await importL1WalletFromMnemonic(walletPageL1, extensionUrl, mnemonicL1);
            await walletPageL1.close();

            // Create page for evm bridge tests
            const page = await context.newPage();
            await page.waitForTimeout(2500); // Wait for the app to load
            await page.goto('/');
            await page.bringToFront();
            // Set up wallet connection
            await connectL1Wallet(page, context);
            await page.waitForTimeout(500);
            await setReceiverAddress(page, addressL2);

            return {
                browser: context,
                page,
                addressL1,
                addressL2,
            };
        };

        // Provide the setup function to the test
        await use(setupFn);
    },
    // L2-only setup (MetaMask)
    browserWithL2Setup: async ({ createContext }, use) => {
        // This function will be provided to each test
        const setupFn = async (testId: string): Promise<BridgeSetupFixture> => {
            console.log('Setting up L2 browser for test:', testId);
            const context = await createContext({
                name: `${testId}-context`,
                extensions: 'l2',
            });

            const extensionId = await waitForExtension(context);
            const extensionUrl = `chrome-extension://${extensionId}/home.html`;

            const state = getSharedState();
            const testData: TestWalletData = state.tests[testId];
            if (!testData) throw new Error(`No test data found for ID: ${testId}`);
            const { addressL2, mnemonicL2, addressL1 } = testData;

            console.log('Setting up L2 browser with extension URL:', extensionUrl);

            // Import/unlock L2 wallet if mnemonic is provided
            console.log('Creating L2 wallet from mnemonic');
            const walletPageL2 = await context.newPage();
            await walletPageL2.waitForTimeout(2500);
            await walletPageL2.goto(extensionUrl);
            await createL2Wallet(walletPageL2, extensionUrl, mnemonicL2);
            await addNetworkToMetaMask(walletPageL2);
            await walletPageL2.close();

            // Create page for evm bridge tests
            const page = await context.newPage();
            await page.waitForTimeout(2500); // Wait for the app to load
            await page.goto('/');
            await page.bringToFront();
            // Set up wallet connection for L2
            await connectL2Wallet(page, context);
            await page.waitForTimeout(500);
            await toggleBridgeDirection(page);
            await page.waitForTimeout(500);
            await setReceiverAddress(page, addressL1);

            return {
                browser: context,
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
