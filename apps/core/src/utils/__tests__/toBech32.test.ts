// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';
import { toBech32 } from '../legacy';

describe('toBech32', () => {
    it('converts a valid hex address to Bech32 format', () => {
        const hexAddress = 'd4555b5b0705cd5e0ba2b1cf8e3e7e0b5a0b1b8dc2e5a3c4e40e5f3d1f3e6c3a';
        const bech32Address = toBech32(hexAddress);

        expect(typeof bech32Address).toBe('string');
        expect(bech32Address.startsWith('iota1')).toBe(true);
    });
});
