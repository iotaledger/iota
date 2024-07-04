// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';
import {
    type AllowedAccountTypes,
    type AllowedBip44CoinTypes,
} from '_src/background/accounts-finder';

export interface SearchAccountsFinderPayload extends BasePayload {
    type: 'search-accounts-finder';
    bip44CoinType: AllowedBip44CoinTypes;
    accountType: AllowedAccountTypes;
    coinType: string;
    sourceID: string;
    accountGapLimit: number;
    addressGapLimit: number;
}

export function isSearchAccountsFinder(payload: Payload): payload is SearchAccountsFinderPayload {
    return isBasePayload(payload) && payload.type === 'search-accounts-finder';
}
