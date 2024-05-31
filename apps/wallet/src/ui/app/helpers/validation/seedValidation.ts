// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { hexToBytes } from '@noble/hashes/utils';
import { z } from 'zod';

export const seedValidation = z
    .string()
    .trim()
    .nonempty('Seed is required.')
    .transform((seed, context) => {
        const hexValue = seed.startsWith('0x') ? seed.slice(2) : seed;
        let seedBytes: Uint8Array | undefined;

        try {
            seedBytes = hexToBytes(hexValue);
        } catch (error) {
            context.addIssue({
                code: 'custom',
                message: 'Invalid Seed, please use 64-byte encoded string.',
            });
            return z.NEVER;
        }

        if (seedBytes.length !== 64) {
            context.addIssue({
                code: 'custom',
                message: 'Hex encoded Seed must be 64 bytes.',
            });
            return z.NEVER;
        }
        return seed;
    });
