import path from 'path';
import { test as base, chromium, type BrowserContext } from '@playwright/test';

const EXTENSION_L1_PATH = path.join(__dirname, '../../wallet-dist-L1');
const EXTENSION_L2_PATH = path.join(__dirname, '../../wallet-dist-L2');

const COMMON_ARGS = ['--user-agent=Playwright', '--disable-dev-shm-usage', '--no-sandbox'];

type ExtensionFixtures = {
    contextL1: BrowserContext;
    contextL2: BrowserContext;
    l1ExtensionUrl: string;
    l2ExtensionUrl: string;
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

export const test = base.extend<ExtensionFixtures>({
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

export const expect = test.expect;
