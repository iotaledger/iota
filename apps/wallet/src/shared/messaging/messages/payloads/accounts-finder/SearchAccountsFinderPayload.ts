// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';
import { type SearchAccountsFinderParams } from '_src/background/accounts-finder';

interface SearchAccountsFinder extends BasePayload {
    type: 'search-accounts-finder';
}

export type SearchAccountsFinderPayload = SearchAccountsFinder & SearchAccountsFinderParams;

export function isSearchAccountsFinder(
    payload: Payload,
): payload is SearchAccountsFinderPayload & SearchAccountsFinderParams {
    return isBasePayload(payload) && payload.type === 'search-accounts-finder';
}
