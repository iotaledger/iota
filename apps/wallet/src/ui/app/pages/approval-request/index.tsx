// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    isSignPersonalMessageApprovalRequest,
    isTransactionApprovalRequest,
} from '_src/shared/messaging/messages/payloads/transactions/approvalRequest';
import { useEffect, useMemo, useRef } from 'react';
import { useParams, useNavigate, useSearchParams } from 'react-router-dom';
import { Loading } from '_components';
import { useAppSelector, useBackgroundClient, useAppDispatch } from '_hooks';
import { type RootState } from '../../redux/rootReducer';
import {
    clearTransactionRequests,
    txRequestsSelectors,
} from '../../redux/slices/transaction-requests';
import { SignMessageRequest } from './SignMessageRequest';
import { TransactionRequest } from './transaction-request';

export function ApprovalRequestPage() {
    const { requestID } = useParams();
    const [searchParams] = useSearchParams();
    const fetch = searchParams.get('fetch');
    const backgroundClient = useBackgroundClient();
    const transactionRequestsLoading = useRef(false);

    const dispatch = useAppDispatch();

    // Wallet fetches pending transaction requests on boot, but because sidepanel
    // stays open we need to manually notify the wallet, to do this we check for a fetch query param
    // If present and equal to true then dispatch a message to fetch the latest transaction requests.
    // We need to dispatch this synchronously rather than async.
    if (!transactionRequestsLoading.current && fetch) {
        dispatch(clearTransactionRequests());
        backgroundClient.sendGetTransactionRequests();
        transactionRequestsLoading.current = true;
    }

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
        if (!requestsLoading && (!request || (request && request.approved !== null))) {
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
