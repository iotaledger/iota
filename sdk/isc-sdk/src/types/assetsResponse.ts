// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';

export const CoinJSONSchema = z.object({
    balance: z.string(),
    coinType: z.string(),
});

export const AssetsResponseSchema = z.object({
    baseTokens: z.string(),
    nativeTokens: z.array(CoinJSONSchema),
});

export type AssetsResponse = z.infer<typeof AssetsResponseSchema>;
