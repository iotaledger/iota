// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import * as z from 'zod';

const Params = z.object({
    name: z.string(),
    timestamp: z.coerce.number(),
});

export function validateParams(params: unknown) {
    return Params.safeParse(params);
}
