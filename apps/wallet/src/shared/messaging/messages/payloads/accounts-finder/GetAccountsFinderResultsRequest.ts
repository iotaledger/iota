// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';

export interface GetAccountsFinderResultsRequest extends BasePayload {
    type: 'get-accounts-finder-results-request';
}

export function isGetAccountsFinderResultsRequest(
    payload: Payload,
): payload is GetAccountsFinderResultsRequest {
    return isBasePayload(payload) && payload.type === 'get-accounts-finder-results-request';
}
