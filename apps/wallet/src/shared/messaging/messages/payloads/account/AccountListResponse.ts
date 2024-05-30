// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type BasePayload, isBasePayload, type Payload } from '_payloads';

export interface AccountListResponse extends BasePayload {
    type: 'account-list-response';
    result: string[];
}

export function isAccountListResponse(payload: Payload): payload is AccountListResponse {
    return isBasePayload(payload) && payload.type === 'disconnect-all-response';
}
