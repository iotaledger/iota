// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';

export const paramsSchema = z.object({
    page: z.preprocess(
        (v: unknown) => (typeof v === 'string' && /^\d+$/.test(v) ? Number(v) : undefined),
        z.number().int().positive().default(1),
    ),
    status: z.preprocess(
        (v: unknown) =>
            typeof v === 'string' && ['all', 'active', 'finished'].includes(v) ? v : undefined,
        z.enum(['all', 'active', 'finished']).default('all'),
    ),
    search: z.preprocess((v: unknown) => (typeof v === 'string' ? v : ''), z.string().default('')),
    size: z.preprocess(
        (v: unknown) => (typeof v === 'string' && /^\d+$/.test(v) ? Number(v) : undefined),
        z.number().int().positive().default(10),
    ),
    sort: z.preprocess(
        (v: unknown) => (typeof v === 'string' && ['asc', 'desc'].includes(v) ? v : undefined),
        z.enum(['asc', 'desc']).default('asc'),
    ),
    sortBy: z.preprocess(
        (v: unknown) =>
            typeof v === 'string' && ['bid', 'name', 'ending'].includes(v) ? v : undefined,
        z.enum(['bid', 'name', 'ending']).default('ending'),
    ),
});
