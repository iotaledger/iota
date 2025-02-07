// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SIZE_LIMIT_EXCEEDED } from '@iota/core';

export function isSizeExceededError(e: Error | null) {
    return e?.message?.includes(SIZE_LIMIT_EXCEEDED);
}
