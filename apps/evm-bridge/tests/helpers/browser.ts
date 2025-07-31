import { Page, type BrowserContext } from '@playwright/test';

export async function getExtensionUrl(browserContext: BrowserContext): Promise<string> {
    let [background] = browserContext.serviceWorkers();

    if (!background) {
        background = await browserContext.waitForEvent('serviceworker', { timeout: 30000 });
    }

    const extensionId = background.url().split('/')[2];
    return `chrome-extension://${extensionId}/ui.html`;
}

export async function waitForExtension(context: BrowserContext): Promise<string> {
    let [background] = context.serviceWorkers();
    if (!background) {
        background = await context.waitForEvent('serviceworker', { timeout: 30000 });
    }

    await new Promise((resolve) => setTimeout(resolve, 1000));

    const extensionId = background.url().split('/')[2];
    return extensionId;
}

export async function waitForExtensions(
    context: BrowserContext,
): Promise<{ l1ExtensionUrl: string; l2ExtensionUrl: string }> {
    // Wait for both extension service workers to start
    const serviceWorkers: string[] = [];
    let l1ExtensionId = '';
    let l2ExtensionId = '';

    // Find extension IDs
    for (const worker of context.serviceWorkers()) {
        const url = worker.url();
        console.log(`Found service worker: ${url}`);
        serviceWorkers.push(url);

        // Check if this is L1 extension (IOTA wallet)
        if (url.includes('background.js')) {
            l1ExtensionId = url.split('/')[2];
        }

        // Check if this is L2 extension (MetaMask)
        if (url.includes('app-init.js')) {
            l2ExtensionId = url.split('/')[2];
        }
    }

    // If extensions are not found, wait for them to load
    if (!l1ExtensionId || !l2ExtensionId) {
        console.log('Waiting for extensions to load...');
        await new Promise((resolve) => setTimeout(resolve, 3000));

        for (const worker of context.serviceWorkers()) {
            const url = worker.url();

            if (url.includes('background.js')) {
                l1ExtensionId = url.split('/')[2];
            }

            if (url.includes('app-init.js')) {
                l2ExtensionId = url.split('/')[2];
            }
        }
    }

    const l1ExtensionUrl = `chrome-extension://${l1ExtensionId}/ui.html`;
    const l2ExtensionUrl = `chrome-extension://${l2ExtensionId}/home.html`;

    return { l1ExtensionUrl, l2ExtensionUrl };
}

/**
 * Create a new page and navigate to URL with error handling
 */
export async function createPage(context: BrowserContext, url = '/'): Promise<Page> {
    try {
        const page = await context.newPage();
        await page.goto(url);
        return page;
    } catch (error) {
        console.error('Failed to create page:', error);
        throw error;
    }
}
