// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type BasePayload, isBasePayload, type Payload } from '_payloads';

export interface DisconnectAllResponse extends BasePayload {
    type: 'disconnect-all-response';
    result: boolean;
}

export function isDisconnectAllResponse(payload: Payload): payload is DisconnectAllResponse {
    return isBasePayload(payload) && payload.type === 'acquire-permissions-response';
}
