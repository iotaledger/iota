// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';

export interface SearchAccountsFinder extends BasePayload {
    type: 'search-accounts-finder';
    coinType: number;
    gasType: string;
    sourceID: string;
    accountGapLimit: number;
    addressGapLimit: number;
}

export function isSearchAccountsFinder(payload: Payload): payload is SearchAccountsFinder {
    return isBasePayload(payload) && payload.type === 'search-accounts-finder';
}
