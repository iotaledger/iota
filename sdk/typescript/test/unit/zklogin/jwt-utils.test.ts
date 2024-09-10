// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, test } from 'vitest';

import { extractClaimValue } from '../../../src/zklogin/jwt-utils';

describe('jwt-utils', () => {
    test('extracts claim value successfully', () => {
        expect(
            extractClaimValue(
                { indexMod4: 1, value: 'yJpc3MiOiJodHRwczovL2FjY291bnRzLmdvb2dsZS5jb20iLC' },
                'iss',
            ),
        ).toBe('https://accounts.google.com');
    });
});
