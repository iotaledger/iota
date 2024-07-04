// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';

export interface ResetAccountsFinder extends BasePayload {
    type: 'reset-accounts-finder';
}

export function isResetAccountsFinder(payload: Payload): payload is ResetAccountsFinder {
    return isBasePayload(payload) && payload.type === 'reset-accounts-finder';
}
