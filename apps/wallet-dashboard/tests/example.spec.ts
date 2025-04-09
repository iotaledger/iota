// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { test, expect } from '@playwright/test';

test('Page Title', async ({ page }) => {
    await page.goto('/');
    const title = await page.title();
    expect(title).toBe('IOTA Wallet Dashboard');
});
