// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { normalizeIotaName } from '@iota/iota-names-sdk';

import { denormalizeName } from '@/lib/utils';

import { expect, test } from '../helpers/fixtures';
import { generateRandomName } from '../utils';
import { addToDenyList } from './validation.utils';

test.describe('Name Validation tests', () => {
    test("Dapp doesn't allow invalid names", async ({ appPage: page }) => {
        const NAME_LENGTH_VALIDATION_ERROR = 'Name must be 3-63 characters long';

        await page.goto('/');
        await page.getByPlaceholder('Search for your IOTA name').click();

        const dialog = page.getByRole('dialog').last();
        const input = dialog.getByPlaceholder('Check name availability');

        const tooLongNames = [
            'a'.repeat(65),
            'b'.repeat(100),
            'loremipsumdolorsitametconsecteturadipiscingelitSeddoeiusmodtemporincididuntutlaboreetdoloremagnaaliqua',
        ];

        for (const tooLongName of tooLongNames) {
            await input.fill(tooLongName);
            await expect(dialog.getByText(NAME_LENGTH_VALIDATION_ERROR)).toBeVisible();
            await input.clear();
        }

        const tooShortNames = ['a', 'ab', 'az'];
        for (const tooShortName of tooShortNames) {
            await input.fill(tooShortName);
            await expect(dialog.getByText(NAME_LENGTH_VALIDATION_ERROR)).toBeVisible();
            await input.clear();
        }

        const invalidCharNames = ['@test', 'one&<!', '(psst)'];
        for (const invalidName of invalidCharNames) {
            await input.fill(invalidName);
            await expect(dialog.getByText('Invalid characters')).toBeVisible();
            await input.clear();
        }
    });

    test("Forbidden names can't be registered", async ({ appPage: page }) => {
        const BLOCKED_NAME = generateRandomName('javascript');
        const normalizedName = normalizeIotaName(BLOCKED_NAME, 'at');

        await addToDenyList([denormalizeName(BLOCKED_NAME)], 'blocked', true);

        await page.goto('/');
        await page.getByPlaceholder('Search for your IOTA name').click();

        const dialog = page.getByRole('dialog').last();
        const input = dialog.getByPlaceholder('Check name availability');

        await input.fill(BLOCKED_NAME);
        await expect(dialog.getByText(normalizedName)).toBeVisible({
            timeout: 10_000,
        });

        await expect(dialog.getByText('Unavailable')).toBeVisible({
            timeout: 10_000,
        });

        await expect(dialog.getByText('Name is blocked and cannot be purchased.')).toBeVisible({
            timeout: 10_000,
        });
    });

    test('Reserved names shows up', async ({ appPage: page }) => {
        const RESERVED_NAME = generateRandomName('tooling');
        await addToDenyList([denormalizeName(RESERVED_NAME)], 'reserved', true);

        await page.goto('/');
        await page.getByPlaceholder('Search for your IOTA name').click();

        const dialog = page.getByRole('dialog').last();
        const input = dialog.getByPlaceholder('Check name availability');

        await input.fill(RESERVED_NAME);
        await expect(dialog.getByText(normalizeIotaName(RESERVED_NAME, 'at'))).toBeVisible({
            timeout: 10_000,
        });

        await expect(dialog.getByText('Reserved')).toBeVisible({
            timeout: 10_000,
        });

        await expect(dialog.getByText('Submit a request to claim.')).toBeVisible({
            timeout: 10_000,
        });
    });
});
