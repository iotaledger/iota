// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';

// Schema for basic output object
export const BasicOutputObjectSchema = z.object({
    content: z.object({
        dataType: z.string(),
        fields: z.object({
            balance: z.string(),
            expiration_uc: z.string(),
            id: z.object({
                id: z.string(),
            }),
            metadata: z.string(),
            native_tokens: z.object({
                fields: z.object({
                    id: z.object({
                        id: z.string(),
                    }),
                    size: z.string(),
                }),
                type: z.string(),
            }),
            sender: z.string(),
            storage_deposit_return_uc: z.object({
                fields: z.object({
                    return_address: z.string(),
                    return_amount: z.string(),
                }),
                type: z.string(),
            }),
            tag: z.string(),
            timelock_uc: z.string(),
        }),
        type: z.string(),
    }),
    digest: z.string(),
    display: z.object({
        data: z.string(),
        error: z.string(),
    }),
    objectId: z.string(),
    type: z.string(),
    version: z.string(),
});
