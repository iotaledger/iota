// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type BasePayload, isBasePayload, type Payload } from '_payloads';

export interface ReconnectForceRequest extends BasePayload {
    type: 'reconnect-force-request';
    origin: string;
}

export function isReconnectForceRequest(payload: Payload): payload is ReconnectForceRequest {
    return isBasePayload(payload) && payload.type === 'reconnect-force-request';
}
