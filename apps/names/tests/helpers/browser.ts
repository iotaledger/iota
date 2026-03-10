// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Page, type BrowserContext } from '@playwright/test';

/**
 * Create a new page and navigate to URL with error handling
 */
export async function createPage(context: BrowserContext, url = '/'): Promise<Page> {
    try {
        const page = await context.newPage();
        await page.goto(url, { waitUntil: 'commit' });
        return page;
    } catch (error) {
        console.error('Failed to create page:', error);
        throw error;
    }
}
