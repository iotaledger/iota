import path from 'path';
import { test as base, chromium, type BrowserContext } from '@playwright/test';

// Path to the wallet extension build directory
const EXTENSION_PATH = path.join(__dirname, '../../wallet/dist');

// Launch arguments that ensure the extension is loaded
const LAUNCH_ARGS = [
    `--disable-extensions-except=${EXTENSION_PATH}`,
    `--load-extension=${EXTENSION_PATH}`,
    // Ensure userAgent is correctly set in serviceworker
    '--user-agent=Playwright',
];

export const test = base.extend<{
    context: BrowserContext;
    extensionUrl: string;
}>({
    // Override the default context to load with the extension
    context: async ({ baseURL }, use) => {
        const context = await chromium.launchPersistentContext('', {
            headless: false,
            args: LAUNCH_ARGS,
        });
        await use(context);
        await context.close();
    },

    // Provide the extension URL to tests
    extensionUrl: async ({ context }, use) => {
        // Get the service worker for the extension
        let [background] = context.serviceWorkers();
        if (!background) {
            background = await context.waitForEvent('serviceworker');
        }

        // Extract extension ID from the service worker URL
        const extensionId = background.url().split('/')[2];
        const extensionUrl = `chrome-extension://${extensionId}/ui.html`;

        await use(extensionUrl);
    },
});

export const expect = test.expect;
