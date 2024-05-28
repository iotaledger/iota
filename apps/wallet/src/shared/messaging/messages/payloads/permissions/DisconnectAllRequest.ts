// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type BasePayload, isBasePayload, type Payload } from '_payloads';

export interface DisconnectAllRequest extends BasePayload {
    type: 'disconnect-all-request';
    origin: string;
}

export function isDisconnectAllRequest(payload: Payload): payload is DisconnectAllRequest {
    return isBasePayload(payload) && payload.type === 'disconnect-all-request';
}
