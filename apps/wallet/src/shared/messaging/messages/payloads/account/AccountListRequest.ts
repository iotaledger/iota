// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type BasePayload, isBasePayload, type Payload } from '_payloads';

export interface AccountListRequest extends BasePayload {
    type: 'account-list-request';
}

export function isAccountListRequest(payload: Payload): payload is AccountListRequest {
    return isBasePayload(payload) && payload.type === 'account-list-request';
}
