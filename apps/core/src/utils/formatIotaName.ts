// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { normalizeIotaName } from '@iota/iota-names-sdk';

export function formatIotaName(name: string | null | undefined): string | null {
    if (!name) return null;
    return normalizeIotaName(name, 'at', {
        onlyFirstSubname: true,
        truncateLongParts: true,
        ellipsisForDeepSubnames: true,
    });
}
