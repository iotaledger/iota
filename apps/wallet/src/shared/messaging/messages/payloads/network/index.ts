// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';
import type { NetworkEnvType } from '_src/shared/api-env';

export interface SetNetworkPayload extends BasePayload {
    type: 'set-network';
    network: NetworkEnvType;
}

export function isSetNetworkPayload(payload: Payload): payload is SetNetworkPayload {
    return isBasePayload(payload) && payload.type === 'set-network';
}
