// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { fromB58 } from '@iota/bcs';
import { decodeIotaPrivateKey, encodeIotaPrivateKey } from '@iota/iota-sdk/cryptography/keypair';
import { hexToBytes } from '@noble/hashes/utils';
import { z } from 'zod';

export const privateKeyValidation = z
    .string()
    .trim()
    .nonempty('Private Key is required.')
    .transform((privateKey, context) => {
        if (!privateKey.startsWith('iotaprivkey')) {
            const hexValue = privateKey.startsWith('0x') ? privateKey.slice(2) : privateKey;
            let privateKeyBytes: Uint8Array | undefined;

            try {
                privateKeyBytes = hexToBytes(hexValue);
            } catch (_) {
                try {
                    // Try decoding base58
                    privateKeyBytes = fromB58(privateKey);
                } catch (_) {
                    context.addIssue({
                        code: 'custom',
                        message:
                            'Invalid Private Key, please use a Bech32 encoded 33-byte string or Base58 encoded string.',
                    });
                    return z.NEVER;
                }
            }

            if (![32, 64].includes(privateKeyBytes.length)) {
                context.addIssue({
                    code: 'custom',
                    message: 'Hex encoded Private Key must be either 32 or 64 bytes.',
                });
                return z.NEVER;
            }

            return encodeIotaPrivateKey(privateKeyBytes.slice(0, 32), 'ED25519');
        }
        try {
            decodeIotaPrivateKey(privateKey);
        } catch (error) {
            context.addIssue({
                code: 'custom',
                message: 'Invalid Private Key, please use a Bech32 encoded 33-byte string',
            });
            return z.NEVER;
        }
        return privateKey;
    });
