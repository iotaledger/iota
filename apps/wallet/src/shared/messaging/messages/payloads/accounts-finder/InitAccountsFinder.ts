// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';

export interface InitAccountsFinder extends BasePayload {
    type: 'init-accounts-finder';
}

export function isInitAccountsFinder(payload: Payload): payload is InitAccountsFinder {
    return isBasePayload(payload) && payload.type === 'init-accounts-finder';
}
