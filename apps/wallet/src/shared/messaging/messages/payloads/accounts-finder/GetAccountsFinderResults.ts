// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';

export interface GetAccountsFinderResults extends BasePayload {
    type: 'get-accounts-finder-results';
    accountGapLimit: number;
}

export function isGetAccountsFinderResults(payload: Payload): payload is GetAccountsFinderResults {
    return isBasePayload(payload) && payload.type === 'get-accounts-finder-results';
}
