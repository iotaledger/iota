// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SignedTransaction } from '_src/ui/app/walletSigner';
import type { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import {
    type IotaSignAndExecuteTransactionInput,
    type IotaSignPersonalMessageOutput,
} from '@iota/wallet-standard';

export type TransactionDataType = {
    type: 'transaction';
    data: string;
    account: string;
    justSign?: boolean;
    options?: IotaSignAndExecuteTransactionInput['options'];
};

export type SignMessageDataType = {
    type: 'sign-personal-message';
    message: string;
    accountAddress: string;
};

export type ApprovalRequest = {
    id: string;
    approved: boolean | null;
    origin: string;
    originFavIcon?: string;
    txResult?: IotaTransactionBlockResponse | IotaSignPersonalMessageOutput;
    txResultError?: string;
    txSigned?: SignedTransaction;
    createdDate: string;
    tx: TransactionDataType | SignMessageDataType;
};

export interface SignPersonalMessageApprovalRequest
    extends Omit<ApprovalRequest, 'txResult' | 'tx'> {
    tx: SignMessageDataType;
    txResult?: IotaSignPersonalMessageOutput;
}

export interface TransactionApprovalRequest extends Omit<ApprovalRequest, 'txResult' | 'tx'> {
    tx: TransactionDataType;
    txResult?: IotaTransactionBlockResponse;
}

export function isSignPersonalMessageApprovalRequest(
    request: ApprovalRequest,
): request is SignPersonalMessageApprovalRequest {
    return request.tx.type === 'sign-personal-message';
}

export function isTransactionApprovalRequest(
    request: ApprovalRequest,
): request is TransactionApprovalRequest {
    return request.tx.type === 'transaction';
}
