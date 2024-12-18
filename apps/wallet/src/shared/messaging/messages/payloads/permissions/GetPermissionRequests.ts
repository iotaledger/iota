// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';

export interface GetPermissionRequests extends BasePayload {
    type: 'get-permission-requests';
}

export function isGetPermissionRequests(payload: Payload): payload is GetPermissionRequests {
    return isBasePayload(payload) && payload.type === 'get-permission-requests';
}
