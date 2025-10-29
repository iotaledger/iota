// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    isSignPersonalMessageApprovalRequest,
    isTransactionApprovalRequest,
} from '_src/shared/messaging/messages/payloads/transactions/approvalRequest';
import { useEffect, useMemo } from 'react';
import { useParams, useNavigate } from 'react-router-dom';
import { Loading } from '_components';
import { useAppSelector } from '_hooks';
import { type RootState } from '../../redux/rootReducer';
import { txRequestsSelectors } from '../../redux/slices/transaction-requests';
import { SignMessageRequest } from './SignMessageRequest';
import { TransactionRequest } from './transaction-request';

export function ApprovalRequestPage() {
    const { requestID } = useParams();
    const navigate = useNavigate();
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
        if (!request && !requestsLoading) {
            navigate('/tokens');
        }
    }, [request, requestsLoading]);

    return (
        <Loading loading={requestsLoading}>
            {request ? (
                isSignPersonalMessageApprovalRequest(request) ? (
                    <SignMessageRequest request={request} />
                ) : isTransactionApprovalRequest(request) ? (
                    <TransactionRequest txRequest={request} />
                ) : null
            ) : null}
        </Loading>
    );
}
