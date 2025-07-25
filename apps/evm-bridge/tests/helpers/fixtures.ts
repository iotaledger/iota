/* eslint-disable no-empty-pattern */
import path from 'path';
import { test as base, chromium, Page, type BrowserContext } from '@playwright/test';
import { closeBrowserTabsExceptLast, waitForExtensions } from './browser';
import {
    connectL1Wallet,
    createL2Wallet,
    addNetworkToMetaMask,
    connectL2Wallet,
    importL1WalletFromMnemonic,
} from './wallet';
import { getSharedState } from './shared-state';

const EXTENSION_L1_PATH = path.join(__dirname, '../../../wallet/dist');
const EXTENSION_L2_PATH = path.join(__dirname, '../../wallet-dist-L2');

const COMMON_ARGS = ['--user-agent=Playwright', '--disable-dev-shm-usage', '--no-sandbox'];

type ExtensionFixtures = {
    context: BrowserContext;
    persistentContext: BrowserContext;
    extensions: { l1ExtensionUrl: string; l2ExtensionUrl: string };
};

type BridgeSetupFixture = {
    browser: BrowserContext;
    page: Page;
    addressL1: string;
    addressL2: string;
};

export const baseTest = base.extend<ExtensionFixtures>({
    context: async ({}, use) => {
        console.log('🔄 Using wallet context');
        const context = await chromium.launchPersistentContext('', {
            headless: false,
            args: [
                ...COMMON_ARGS,
                `--disable-extensions-except=${EXTENSION_L1_PATH},${EXTENSION_L2_PATH}`,
                `--load-extension=${EXTENSION_L1_PATH},${EXTENSION_L2_PATH}`,
            ],
        });

        await use(context);
        await context.close();
    },
    persistentContext: async ({}, use) => {
        console.log('🔄 Usingpersistent wallet context');
        const context = await chromium.launchPersistentContext('', {
            headless: false,
            args: [
                ...COMMON_ARGS,
                `--disable-extensions-except=${EXTENSION_L1_PATH},${EXTENSION_L2_PATH}`,
                `--load-extension=${EXTENSION_L1_PATH},${EXTENSION_L2_PATH}`,
            ],
        });

        await use(context);
    },

    extensions: async ({ context }, use) => {
        const extensions = await waitForExtensions(context);
        await use(extensions);
    },
});

// Create a generic setup fixture
export const test = baseTest.extend<{
    browserSetup: (testId: string, persistent?: boolean) => Promise<BridgeSetupFixture>;
}>({
    browserSetup: async ({ context, persistentContext, extensions }, use) => {
        // This function will be provided to each test
        const setupFn = async (
            testId: string,
            persistent?: boolean,
        ): Promise<BridgeSetupFixture> => {
            console.log('Setting up browser for test:', testId);
            // Determine which context to use
            const usePersistent = persistent ?? false;
            const activeContext = usePersistent ? persistentContext : context;
            console.log(usePersistent ? 'Using persistent context' : 'Using auto-closing context');

            const state = getSharedState();
            const testData = state.tests[testId];
            if (!testData) throw new Error(`No test data found for ID: ${testId}`);
            const { addressL1, addressL2, mnemonicL1, mnemonicL2 } = testData;
            const { l1ExtensionUrl, l2ExtensionUrl } = extensions;
            console.log('Setting up browser l1ExtensionUrl:', l1ExtensionUrl);
            console.log('Setting up browser l2ExtensionUrl:', l2ExtensionUrl);
            // Import/unlock wallets if mnemonics are provided
            if (mnemonicL1) {
                const walletPageL1 = await activeContext.newPage();
                await walletPageL1.goto(l1ExtensionUrl);
                await closeBrowserTabsExceptLast(activeContext);
                await importL1WalletFromMnemonic(walletPageL1, l1ExtensionUrl, mnemonicL1);
                await walletPageL1.close();
            }

            if (mnemonicL2) {
                const walletPageL2 = await activeContext.newPage();
                await walletPageL2.goto(l2ExtensionUrl);
                await closeBrowserTabsExceptLast(activeContext);
                await createL2Wallet(walletPageL2, l2ExtensionUrl, mnemonicL2);
                await addNetworkToMetaMask(walletPageL2);
                await walletPageL2.close();
            }

            // Create page for evm bridge tests
            const page = await activeContext.newPage();
            await page.goto('/');
            // await closeBrowserTabsExceptLast(context);
            // Set up wallet connections
            await page.waitForTimeout(2500); // Wait for the app to load
            await connectL1Wallet(page, activeContext);
            await page.waitForTimeout(2500);
            await connectL2Wallet(page, activeContext);

            return {
                browser: activeContext,
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
