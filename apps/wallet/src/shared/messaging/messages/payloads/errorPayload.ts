// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Payload } from './payload';

export interface ErrorPayload {
    error: true;
    code: number;
    message: string;
}

export function isErrorPayload(payload: Payload): payload is ErrorPayload {
    return 'error' in payload && payload.error === true;
}
