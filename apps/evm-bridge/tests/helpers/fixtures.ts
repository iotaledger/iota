/* eslint-disable no-empty-pattern */
import path from 'path';
import { test as base, chromium, Page, type BrowserContext } from '@playwright/test';
import { createPage, waitForExtension, waitForExtensions } from './browser';
import {
    connectL1Wallet,
    connectL2Wallet,
    setupBridgeWallets,
    setupL1Wallet,
    setupL2Wallet,
    waitForL1WalletConnected,
} from './wallet';
import { getTestData, WalletState } from './shared-state';
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

            // If not persistent, register a finalizer to close the context when test is done
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

export const test = baseTest.extend<{
    browserWithBothExtensionsSetup: (
        testId: keyof WalletState['tests'],
    ) => Promise<BridgeSetupFixture>;
    browserWithL1Setup: (testId: keyof WalletState['tests']) => Promise<BridgeSetupFixture>;
    browserWithL2Setup: (testId: keyof WalletState['tests']) => Promise<BridgeSetupFixture>;
}>({
    // Both L1 and L2 setup
    browserWithBothExtensionsSetup: async ({ createContext }, use) => {
        const setupFn = async (testId: keyof WalletState['tests']): Promise<BridgeSetupFixture> => {
            console.log('Setting up browser for test:', testId);
            const context = await createContext({
                persistent: true,
                name: `${testId}-context`,
                extensions: 'both',
            });

            const { l1ExtensionUrl, l2ExtensionUrl } = await waitForExtensions(context);

            const testData: TestWalletData = getTestData(testId);
            if (!testData) throw new Error(`No test data found for ID: ${testId}`);
            const { addressL1, addressL2, mnemonicL1, mnemonicL2 } = testData;

            await setupBridgeWallets(
                context,
                l1ExtensionUrl,
                l2ExtensionUrl,
                mnemonicL1,
                mnemonicL2,
                testId,
            );

            const page = await createPage(context);
            await page.bringToFront();

            await connectL1Wallet(page, context);
            await waitForL1WalletConnected(page, { timeout: 30000 }, testId);

            await connectL2Wallet(page, context);

            return {
                browser: context,
                page,
                addressL1,
                addressL2,
            };
        };

        await use(setupFn);
    },
    // L1-only setup (IOTA Wallet)
    browserWithL1Setup: async ({ createContext }, use) => {
        const setupFn = async (testId: keyof WalletState['tests']): Promise<BridgeSetupFixture> => {
            console.log('Setting up L1 browser for test:', testId);
            const context = await createContext({
                name: `${testId}-context`,
                extensions: 'l1',
            });

            const extensionId = await waitForExtension(context);
            const extensionUrl = `chrome-extension://${extensionId}/ui.html`;

            const testData: TestWalletData = getTestData(testId);
            if (!testData) throw new Error(`No test data found for ID: ${testId}`);
            const { addressL1, mnemonicL1, addressL2 } = testData;

            await setupL1Wallet(context, extensionUrl, mnemonicL1, testId);

            const page = await createPage(context);
            await page.bringToFront();

            await connectL1Wallet(page, context);
            await waitForL1WalletConnected(page, { timeout: 30000 }, testId);
            await setReceiverAddress(page, addressL2);

            return {
                browser: context,
                page,
                addressL1,
                addressL2,
            };
        };

        await use(setupFn);
    },
    // L2-only setup (MetaMask)
    browserWithL2Setup: async ({ createContext }, use) => {
        const setupFn = async (testId: keyof WalletState['tests']): Promise<BridgeSetupFixture> => {
            console.log('Setting up L2 browser for test:', testId);
            const context = await createContext({
                name: `${testId}-context`,
                extensions: 'l2',
            });

            const extensionId = await waitForExtension(context);
            const extensionUrl = `chrome-extension://${extensionId}/home.html`;

            const testData: TestWalletData = getTestData(testId);
            if (!testData) throw new Error(`No test data found for ID: ${testId}`);
            const { addressL2, mnemonicL2, addressL1 } = testData;

            await setupL2Wallet(context, extensionUrl, mnemonicL2, testId);

            const page = await createPage(context);
            await page.bringToFront();
            await page.waitForLoadState('networkidle');

            await connectL2Wallet(page, context);
            await toggleBridgeDirection(page);
            await setReceiverAddress(page, addressL1);

            return {
                browser: context,
                page,
                addressL1,
                addressL2,
            };
        };

        await use(setupFn);
    },
});

export const expect = test.expect;
