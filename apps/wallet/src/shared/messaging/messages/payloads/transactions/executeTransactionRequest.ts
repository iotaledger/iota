// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isBasePayload } from '_payloads';
import type { BasePayload, Payload } from '_payloads';
import { type IotaSignTransactionInput } from '@iota/wallet-standard';

import { type TransactionDataType } from './approvalRequest';

export interface ExecuteTransactionRequest extends BasePayload {
    type: 'execute-transaction-request';
    transaction: TransactionDataType;
}

export function isExecuteTransactionRequest(
    payload: Payload,
): payload is ExecuteTransactionRequest {
    return isBasePayload(payload) && payload.type === 'execute-transaction-request';
}

export type IotaSignTransactionSerialized = Omit<
    IotaSignTransactionInput,
    'transaction' | 'account'
> & {
    transaction: string;
    account: string;
};

export interface SignTransactionRequest extends BasePayload {
    type: 'sign-transaction-request';
    transaction: IotaSignTransactionSerialized;
}

export function isSignTransactionRequest(payload: Payload): payload is SignTransactionRequest {
    return isBasePayload(payload) && payload.type === 'sign-transaction-request';
}
