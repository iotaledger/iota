// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { Payload } from './payload';

export type PayloadType =
    | 'permission-request'
    | 'permission-response'
    | 'get-permission-requests'
    | 'get-account'
    | 'get-account-response'
    | 'has-permissions-request'
    | 'has-permissions-response'
    | 'acquire-permissions-request'
    | 'acquire-permissions-response'
    | 'execute-transaction-request'
    | 'execute-transaction-response'
    | 'sign-transaction-request'
    | 'sign-transaction-response'
    | 'get-transaction-requests'
    | 'get-transaction-requests-response'
    | 'transaction-request-response'
    | 'update-active-origin'
    | 'disconnect-app'
    | 'done'
    | 'keyring'
    | 'wallet-status-changed'
    | 'get-features'
    | 'features-response'
    | 'get-network'
    | 'set-network'
    | 'sign-personal-message-request'
    | 'method-payload'
    | 'derive-bip-path-accounts-finder'
    | 'derive-bip-path-accounts-finder-response'
    | 'persist-accounts-finder';

export interface BasePayload {
    type: PayloadType;
}

export function isBasePayload(payload: Payload): payload is BasePayload {
    return 'type' in payload && typeof payload.type !== 'undefined';
}
