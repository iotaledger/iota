// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';
import { normalizeIotaAddress } from '../../../src/utils/iota-types';

describe('normalizeIotaAddress', () => {
    it('should convert address to lowercase', () => {
        expect(normalizeIotaAddress('0xABC123')).toBe(
            '0x0000000000000000000000000000000000000000000000000000000000abc123',
        );
    });

    it('should remove whitespaces', () => {
        expect(normalizeIotaAddress(' 0xab  c1 23 ')).toBe(
            '0x0000000000000000000000000000000000000000000000000000000000abc123',
        );
    });

    it('should add 0x prefix when missing', () => {
        expect(normalizeIotaAddress('abc123')).toBe(
            '0x0000000000000000000000000000000000000000000000000000000000abc123',
        );
    });

    it('should pad address to correct length', () => {
        expect(normalizeIotaAddress('0x000123')).toBe(
            '0x0000000000000000000000000000000000000000000000000000000000000123',
        );
    });

    it('should add 0x prefix when forceAdd0x is true', () => {
        expect(normalizeIotaAddress('0x00123', true)).toBe(
            '0x0000000000000000000000000000000000000000000000000000000000x00123',
        );
    });

    it('should handle empty string', () => {
        expect(normalizeIotaAddress('')).toBe(
            '0x0000000000000000000000000000000000000000000000000000000000000000',
        );
    });

    it('should validate address when validate flag is true', () => {
        expect(normalizeIotaAddress('0x000123', false, true)).toBe(
            '0x0000000000000000000000000000000000000000000000000000000000000123',
        );
    });

    it('should throw error when validate flag is true and address is invalid', () => {
        expect(() => normalizeIotaAddress('0xabc123g', false, true)).toThrowError(
            'Invalid IOTA address: 0xabc123g',
        );
    });
});
