// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CONFIG } from '@/config';

export function getNameDisplaySrc(name: string, expirationTimestamp: number) {
    const baseUrl = CONFIG.namesDisplayUrl;
    return new URL(`${name}/${expirationTimestamp}`, baseUrl).href;
}
