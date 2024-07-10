// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { BasePayload } from '_payloads';
import { type AddressFromFinder } from '_src/shared/accounts';

export interface GetAccountsFinderResultsResponse extends BasePayload {
    type: 'get-accounts-finder-results-response';
    results: AddressFromFinder[];
}
