// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';

export interface SidepanelSetState extends BasePayload {
    type: 'sidepanel-set-state';
    open: boolean;
}

export function isSidepanelSetState(payload: Payload): payload is SidepanelSetState {
    return isBasePayload(payload) && payload.type === 'sidepanel-set-state';
}
