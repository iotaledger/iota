// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SignedTransaction } from '_src/ui/app/WalletSigner';
import type { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import {
    type IotaSignAndExecuteTransactionBlockInput,
    type IotaSignMessageOutput,
} from '@iota/wallet-standard';

export type TransactionDataType = {
    type: 'transaction';
    data: string;
    account: string;
    justSign?: boolean;
    requestType?: IotaSignAndExecuteTransactionBlockInput['requestType'];
    options?: IotaSignAndExecuteTransactionBlockInput['options'];
};

export type SignMessageDataType = {
    type: 'sign-message';
    message: string;
    accountAddress: string;
};

export type ApprovalRequest = {
    id: string;
    approved: boolean | null;
    origin: string;
    originFavIcon?: string;
    txResult?: IotaTransactionBlockResponse | IotaSignMessageOutput;
    txResultError?: string;
    txSigned?: SignedTransaction;
    createdDate: string;
    tx: TransactionDataType | SignMessageDataType;
};

export interface SignMessageApprovalRequest extends Omit<ApprovalRequest, 'txResult' | 'tx'> {
    tx: SignMessageDataType;
    txResult?: IotaSignMessageOutput;
}

export interface TransactionApprovalRequest extends Omit<ApprovalRequest, 'txResult' | 'tx'> {
    tx: TransactionDataType;
    txResult?: IotaTransactionBlockResponse;
}

export function isSignMessageApprovalRequest(
    request: ApprovalRequest,
): request is SignMessageApprovalRequest {
    return request.tx.type === 'sign-message';
}

export function isTransactionApprovalRequest(
    request: ApprovalRequest,
): request is TransactionApprovalRequest {
    return request.tx.type !== 'sign-message';
}
