// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    isSignMessageApprovalRequest,
    isTransactionApprovalRequest,
} from '_payloads/transactions/ApprovalRequest';
import { useEffect, useMemo } from 'react';
import { useParams } from 'react-router-dom';

import Loading from '_components/loading';
import { useAppSelector } from '_app/hooks';
import { type RootState } from '_redux/RootReducer';
import { txRequestsSelectors } from '_redux/slices/transaction-requests';
import { SignMessageRequest } from './SignMessageRequest';
import { TransactionRequest } from './transaction-request';

export function ApprovalRequestPage() {
    const { requestID } = useParams();
    const requestSelector = useMemo(
        () => (state: RootState) =>
            (requestID && txRequestsSelectors.selectById(state, requestID)) || null,
        [requestID],
    );
    const request = useAppSelector(requestSelector);
    const requestsLoading = useAppSelector(
        ({ transactionRequests }) => !transactionRequests.initialized,
    );
    useEffect(() => {
        if (!requestsLoading && (!request || (request && request.approved !== null))) {
            window.close();
        }
    }, [requestsLoading, request]);
    return (
        <Loading loading={requestsLoading}>
            {request ? (
                isSignMessageApprovalRequest(request) ? (
                    <SignMessageRequest request={request} />
                ) : isTransactionApprovalRequest(request) ? (
                    <TransactionRequest txRequest={request} />
                ) : null
            ) : null}
        </Loading>
    );
}
