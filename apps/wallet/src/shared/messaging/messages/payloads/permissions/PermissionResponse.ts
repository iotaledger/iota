// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';

export interface PermissionResponse extends BasePayload {
    type: 'permission-response';
    id: string;
    accounts: string[];
    allowed: boolean;
    responseDate: string;
}

export function isPermissionResponse(payload: Payload): payload is PermissionResponse {
    return isBasePayload(payload) && payload.type === 'permission-response';
}
