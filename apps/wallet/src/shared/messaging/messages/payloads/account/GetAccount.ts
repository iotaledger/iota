// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';

export interface GetAccount extends BasePayload {
    type: 'get-account';
}

export function isGetAccount(payload: Payload): payload is GetAccount {
    return isBasePayload(payload) && payload.type === 'get-account';
}
