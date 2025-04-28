// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';

// Schema for validator object
export const ValidatorSchema = z.object({
    fields: z.object({
        name: z.string(),
        value: z.object({
            fields: z.object({
                inner: z.object({
                    fields: z.object({
                        id: z.object({
                            id: z.string(),
                        }),
                    }),
                }),
            }),
        }),
    }),
});

// Schema for dynamic field object
export const DynamicFieldObjectSchema = z.object({
    fields: z.object({
        value: z.object({
            fields: z.object({
                metadata: z.object({
                    fields: z.object({
                        image_url: z.string(),
                        name: z.string(),
                        description: z.string(),
                        project_url: z.string(),
                        protocol_pubkey_bytes: z.array(z.number()),
                        iota_address: z.string(),
                    }),
                }),
            }),
        }),
    }),
});
