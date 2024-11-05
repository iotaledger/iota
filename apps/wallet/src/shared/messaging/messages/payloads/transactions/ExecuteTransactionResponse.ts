// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';
import type { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { type IotaSignTransactionOutput } from '@iota/wallet-standard';

export interface ExecuteTransactionResponse extends BasePayload {
    type: 'execute-transaction-response';
    result: IotaTransactionBlockResponse;
}

export function isExecuteTransactionResponse(
    payload: Payload,
): payload is ExecuteTransactionResponse {
    return isBasePayload(payload) && payload.type === 'execute-transaction-response';
}

export interface SignTransactionResponse extends BasePayload {
    type: 'sign-transaction-response';
    result: IotaSignTransactionOutput;
}

export function isSignTransactionResponse(payload: Payload): payload is SignTransactionResponse {
    return isBasePayload(payload) && payload.type === 'sign-transaction-response';
}
