// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';

export const ChainDataSchema = z.object({
    packageId: z.string(),
    chainId: z.string(),
});

export type ChainData = z.infer<typeof ChainDataSchema>;
