// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';

import type { Permission } from './permission';

export interface PermissionRequests extends BasePayload {
    type: 'permission-request';
    permissions: Permission[];
}

export function isPermissionRequests(payload: Payload): payload is PermissionRequests {
    return isBasePayload(payload) && payload.type === 'permission-request';
}
