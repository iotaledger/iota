// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, it, expect } from 'vitest';
import { normalizeIotaAddress } from '@iota/iota-sdk/utils';

// Some dummy test to ensure the pnpm test command is not failing due to CI
describe('@iota/signers', () => {
    it('should normalize IOTA addresses correctly', () => {
        const shortAddr = normalizeIotaAddress('0x2');
        const customAddr = normalizeIotaAddress('0x123');

        expect(shortAddr).toBe(
            '0x0000000000000000000000000000000000000000000000000000000000000002',
        );
        expect(customAddr).toBe(
            '0x0000000000000000000000000000000000000000000000000000000000000123',
        );
        expect(shortAddr).toHaveLength(66); // 0x + 64 hex chars
    });

    it('should handle base64 encoding for BCS data', () => {
        const testData = new Uint8Array([1, 2, 3, 4, 5]);
        const encoded = Buffer.from(testData).toString('base64');
        const decoded = new Uint8Array(Buffer.from(encoded, 'base64'));

        expect(encoded).toBe('AQIDBAU=');
        expect(decoded).toEqual(testData);
        expect(decoded.length).toBe(5);
    });
});
