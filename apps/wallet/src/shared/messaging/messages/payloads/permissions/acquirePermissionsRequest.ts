// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';

import type { PermissionType } from './permissionType';

export interface AcquirePermissionsRequest extends BasePayload {
    type: 'acquire-permissions-request';
    permissions: readonly PermissionType[];
}

export function isAcquirePermissionsRequest(
    payload: Payload,
): payload is AcquirePermissionsRequest {
    return isBasePayload(payload) && payload.type === 'acquire-permissions-request';
}
