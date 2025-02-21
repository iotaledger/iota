// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { ApprovalRequest } from '_src/shared/messaging/messages/payloads/transactions/approvalRequest';
import type { RootState } from '_src/ui/app/redux/rootReducer';
import { getSignerOperationErrorMessage } from '_src/ui/app/helpers/errorMessages';
import {
    type SignedMessage,
    type SignedTransaction,
    type WalletSigner,
} from '_src/ui/app/walletSigner';
import type { AppThunkConfig } from '_src/ui/app/redux/store/thunkExtras';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { Transaction } from '@iota/iota-sdk/transactions';
import { fromB64 } from '@iota/iota-sdk/utils';
import { createAsyncThunk, createEntityAdapter, createSlice } from '@reduxjs/toolkit';
import type { PayloadAction } from '@reduxjs/toolkit';

const txRequestsAdapter = createEntityAdapter<ApprovalRequest>({
    sortComparer: (a, b) => {
        const aDate = new Date(a.createdDate);
        const bDate = new Date(b.createdDate);
        return aDate.getTime() - bDate.getTime();
    },
});

export const respondToTransactionRequest = createAsyncThunk<
    {
        txRequestID: string;
        approved: boolean;
        txResponse: IotaTransactionBlockResponse | null;
    },
    {
        txRequestID: string;
        approved: boolean;
        signer: WalletSigner;
    },
    AppThunkConfig
>(
    'respond-to-transaction-request',
    async ({ txRequestID, approved, signer }, { extra: { background }, getState }) => {
        const state = getState();
        const txRequest = txRequestsSelectors.selectById(state, txRequestID);
        if (!txRequest) {
            throw new Error(`TransactionRequest ${txRequestID} not found`);
        }
        let txSigned: SignedTransaction | undefined = undefined;
        let txResult: IotaTransactionBlockResponse | SignedMessage | undefined = undefined;
        let txResultError: string | undefined;
        if (approved) {
            try {
                if (txRequest.tx.type === 'sign-personal-message') {
                    txResult = await signer.signMessage({
                        message: fromB64(txRequest.tx.message),
                    });
                } else if (txRequest.tx.type === 'transaction') {
                    const tx = Transaction.from(txRequest.tx.data);
                    if (txRequest.tx.justSign) {
                        // Just a signing request, do not submit
                        txSigned = await signer.signTransaction({
                            transaction: tx,
                        });
                    } else {
                        txResult = await signer.signAndExecuteTransaction({
                            transactionBlock: tx,
                            options: txRequest.tx.options,
                        });
                    }
                } else {
                    throw new Error(
                        // eslint-disable-next-line @typescript-eslint/no-explicit-any
                        `Unexpected type: ${(txRequest.tx as any).type}`,
                    );
                }
            } catch (error) {
                txResultError = getSignerOperationErrorMessage(error);
            }
        }
        background.sendTransactionRequestResponse(
            txRequestID,
            approved,
            txResult,
            txResultError,
            txSigned,
        );
        return { txRequestID, approved: approved, txResponse: null };
    },
);

const slice = createSlice({
    name: 'transaction-requests',
    initialState: txRequestsAdapter.getInitialState({
        initialized: false,
    }),
    reducers: {
        setTransactionRequests: (state, { payload }: PayloadAction<ApprovalRequest[]>) => {
            // eslint-disable-next-line @typescript-eslint/ban-ts-comment
            // @ts-ignore
            txRequestsAdapter.setAll(state, payload);
            state.initialized = true;
        },
    },
    extraReducers: (build) => {
        build.addCase(respondToTransactionRequest.fulfilled, (state, { payload }) => {
            const { txRequestID, approved: allowed, txResponse } = payload;
            txRequestsAdapter.updateOne(state, {
                id: txRequestID,
                changes: {
                    approved: allowed,
                    txResult: txResponse || undefined,
                },
            });
        });
    },
});

export default slice.reducer;

export const { setTransactionRequests } = slice.actions;

export const txRequestsSelectors = txRequestsAdapter.getSelectors(
    (state: RootState) => state.transactionRequests,
);
