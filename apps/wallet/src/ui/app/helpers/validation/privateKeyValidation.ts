// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { decodeIotaPrivateKey } from '@iota/iota-sdk/cryptography/keypair';
import { z } from 'zod';

export const privateKeyValidation = z
    .string()
    .trim()
    .nonempty('Private Key is required.')
    .transform((privateKey, context) => {
        try {
            decodeIotaPrivateKey(privateKey);
        } catch (error) {
            context.addIssue({
                code: 'custom',
                message:
                    'Invalid Private Key, please use a Bech32 encoded 33-byte string. Learn more: https://github.com/sui-foundation/sips/blob/main/sips/sip-15.md',
            });
            return z.NEVER;
        }
        return privateKey;
    });
