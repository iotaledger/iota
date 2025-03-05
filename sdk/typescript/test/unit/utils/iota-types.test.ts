// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';
import { normalizeIotaAddress } from '../../../src/utils/iota-types';

describe('normalizeIotaAddress', () => {
    it('should convert address to lowercase', () => {
        expect(normalizeIotaAddress('0xABC123')).toBe(
            '0x0000000000000000000000000000000000000000000000000000000000abc123',
        );
    });

    it('should trim whitespace', () => {
        expect(normalizeIotaAddress(' 0xabc123 ')).toBe(
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

    it('should remove 0x prefix and then add it back by default', () => {
        expect(normalizeIotaAddress('0x0x123')).toBe(
            '0x00000000000000000000000000000000000000000000000000000000000123',
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

    it('should throw error if address is not a valid hex', () => {
        expect(() => normalizeIotaAddress('0xabc123g')).toThrowError(
            'Invalid address format: "0xabc123g". Address must contain only hexadecimal characters.',
        );
    });
});
