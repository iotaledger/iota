/* eslint-disable no-empty-pattern */
import path from 'path';
import { test as base, chromium, Page, type BrowserContext } from '@playwright/test';
import { closeBrowserTabsExceptLast } from './browser';
import { setReceiverAddress, toggleBridgeDirection } from './ui';
import {
    connectL1Wallet,
    connectL2Wallet,
    unlockIOTAWallet,
    unlockMetaMask,
    isL1WalletConnected,
    isL2WalletConnected,
} from './wallet';
import { getSharedState, getUserDataPaths } from './shared-state';

const EXTENSION_L1_PATH = path.join(__dirname, '../../../wallet/dist');
const EXTENSION_L2_PATH = path.join(__dirname, '../../wallet-dist-L2');

const COMMON_ARGS = ['--user-agent=Playwright', '--disable-dev-shm-usage', '--no-sandbox'];

type ExtensionFixtures = {
    contextL1: BrowserContext;
    contextL2: BrowserContext;
    l1ExtensionUrl: string;
    l2ExtensionUrl: string;
};

type BridgeSetupFixture = {
    browserL1: BrowserContext;
    browserL2: BrowserContext;
    pageWithL1Wallet: Page;
    pageWithL2Wallet: Page;
    addressL1: string;
    addressL2: string;
};

export const baseTest = base.extend<ExtensionFixtures>({
    contextL1: async ({}, use) => {
        const paths = getUserDataPaths();

        console.log('🔄 Using shared L1 wallet context');
        const context = await chromium.launchPersistentContext(paths.userDataDirL1, {
            headless: false,
            args: [
                ...COMMON_ARGS,
                `--disable-extensions-except=${EXTENSION_L1_PATH}`,
                `--load-extension=${EXTENSION_L1_PATH}`,
            ],
        });

        await use(context);
        // await context.close();
    },

    contextL2: async ({}, use) => {
        const paths = getUserDataPaths();

        console.log('🔄 Using shared L2 wallet context');
        const context = await chromium.launchPersistentContext(paths.userDataDirL2, {
            headless: false,
            args: [
                ...COMMON_ARGS,
                `--disable-extensions-except=${EXTENSION_L2_PATH}`,
                `--load-extension=${EXTENSION_L2_PATH}`,
            ],
        });

        await use(context);
        // await context.close();
    },

    l1ExtensionUrl: async ({}, use) => {
        const state = getSharedState();
        const extensionUrl = `chrome-extension://${state.extensionIdL1}/ui.html`;
        await use(extensionUrl);
    },

    l2ExtensionUrl: async ({}, use) => {
        const state = getSharedState();
        const extensionUrl = `chrome-extension://${state.extensionIdL2}/home.html`;
        await use(extensionUrl);
    },
});

export const test = baseTest.extend<{
    roundtripSetup: BridgeSetupFixture;
}>({
    // Full roundtrip setup with both L1 and L2
    roundtripSetup: async ({ contextL1, l1ExtensionUrl, contextL2, l2ExtensionUrl }, use) => {
        const state = getSharedState();
        const addressL1 = state.addressL1;
        const addressL2 = state.addressL2;

        console.log('📝 Using global wallet addresses:');
        console.log(`   L1: ${addressL1}`);
        console.log(`   L2: ${addressL2}`);

        // Keep wallets unlocked by opening and unlocking
        const extensionPageL1 = await contextL1.newPage();
        await extensionPageL1.goto(l1ExtensionUrl);
        await unlockIOTAWallet(extensionPageL1);
        await extensionPageL1.close();

        const extensionPageL2 = await contextL2.newPage();
        await extensionPageL2.waitForTimeout(500); // Wait for extension to load
        await extensionPageL2.goto(l2ExtensionUrl);
        await unlockMetaMask(extensionPageL2);
        await extensionPageL2.close();

        // Create pages for testing
        const pageWithL1Wallet = await contextL1.newPage();
        const pageWithL2Wallet = await contextL2.newPage();

        // Go to app URL
        await pageWithL1Wallet.goto('/');
        await pageWithL2Wallet.goto('/');
        await closeBrowserTabsExceptLast(contextL1);
        await closeBrowserTabsExceptLast(contextL2);

        // Set up wallet connections
        if (!(await isL1WalletConnected(pageWithL1Wallet))) {
            console.log('Connecting L1 wallet...');
            await connectL1Wallet(pageWithL1Wallet, contextL1);
        }
        await setReceiverAddress(pageWithL1Wallet, addressL2);

        if (!(await isL2WalletConnected(pageWithL2Wallet))) {
            console.log('Connecting L2 wallet...');
            await connectL2Wallet(pageWithL2Wallet, contextL2);
        }
        await toggleBridgeDirection(pageWithL2Wallet);
        await setReceiverAddress(pageWithL2Wallet, addressL1);

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
