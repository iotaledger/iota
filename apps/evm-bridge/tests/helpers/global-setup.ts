// global-setup.ts
import { chromium } from '@playwright/test';
import { existsSync, writeFileSync, mkdirSync } from 'fs';
import path from 'path';
import { generate24WordMnemonic, deriveAddressFromMnemonic } from '../utils/utils';
import {
    importL1WalletFromMnemonic,
    createL2Wallet,
    addNetworkToMetaMask,
    getRandomL2MnemonicAndAddress,
} from './wallet';
import { waitForExtension } from './browser';
import { STATE_FILE, STATE_DIR, USER_DATA_DIR_L1, USER_DATA_DIR_L2 } from './paths';

// Extensions paths
const EXTENSION_L1_PATH = path.join(__dirname, '../../../wallet/dist');
const EXTENSION_L2_PATH = path.join(__dirname, '../../wallet-dist-L2');
const COMMON_ARGS = ['--user-agent=Playwright', '--disable-dev-shm-usage', '--no-sandbox'];

async function globalSetup() {
    // Create state directories
    if (!existsSync(STATE_DIR)) mkdirSync(STATE_DIR, { recursive: true });
    if (!existsSync(USER_DATA_DIR_L1)) mkdirSync(USER_DATA_DIR_L1, { recursive: true });
    if (!existsSync(USER_DATA_DIR_L2)) mkdirSync(USER_DATA_DIR_L2, { recursive: true });

    // Launch persistent contexts for L1 and L2
    const browserL1 = await chromium.launchPersistentContext(USER_DATA_DIR_L1, {
        headless: false,
        args: [
            ...COMMON_ARGS,
            `--disable-extensions-except=${EXTENSION_L1_PATH}`,
            `--load-extension=${EXTENSION_L1_PATH}`,
        ],
    });

    const browserL2 = await chromium.launchPersistentContext(USER_DATA_DIR_L2, {
        headless: false,
        args: [
            ...COMMON_ARGS,
            `--disable-extensions-except=${EXTENSION_L2_PATH}`,
            `--load-extension=${EXTENSION_L2_PATH}`,
        ],
    });

    try {
        // Get extension URLs
        const extensionIdL1 = await waitForExtension(browserL1);
        const extensionIdL2 = await waitForExtension(browserL2);

        const l1ExtensionUrl = `chrome-extension://${extensionIdL1}/ui.html`;
        const l2ExtensionUrl = `chrome-extension://${extensionIdL2}/home.html`;

        // Create L1 wallet
        const mnemonicL1 = generate24WordMnemonic();
        const pageL1 = await browserL1.newPage();
        await importL1WalletFromMnemonic(pageL1, l1ExtensionUrl, mnemonicL1);
        const addressL1 = deriveAddressFromMnemonic(mnemonicL1);
        await pageL1.close();

        // Create L2 wallet
        const { mnemonic: mnemonicL2, address: addressL2 } = getRandomL2MnemonicAndAddress();

        const pageL2 = await browserL2.newPage();
        await createL2Wallet(pageL2, l2ExtensionUrl, mnemonicL2);
        await addNetworkToMetaMask(pageL2);
        await pageL2.waitForTimeout(5000);
        await pageL2.close();

        // Save state to file
        const state = {
            extensionIdL1,
            extensionIdL2,
            addressL1,
            addressL2,
            mnemonicL1,
            mnemonicL2,
            createdAt: new Date().toISOString(),
        };

        writeFileSync(STATE_FILE, JSON.stringify(state, null, 2));
    } finally {
        // Close browsers after setup
        await browserL1.close();
        await browserL2.close();
    }
}

export default globalSetup;
