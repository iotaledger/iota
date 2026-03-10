// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';

export const envSchema = z.record(
    z.string(),
    z.object({
        network: z.enum(['localnet', 'devnet', 'testnet', 'mainnet']),
        indexerUrl: z.string(),
        namesDisplayUrl: z.string(),
        baseUrl: z.string(),
    }),
);

export type Config = z.infer<typeof envSchema>;
