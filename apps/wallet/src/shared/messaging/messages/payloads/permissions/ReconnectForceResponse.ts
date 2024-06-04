// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type BasePayload } from '_payloads';

export interface ReconnectForceResponse extends BasePayload {
    type: 'reconnect-force-response';
    result: boolean;
}
