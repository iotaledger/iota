// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    isSignPersonalMessageApprovalRequest,
    isTransactionApprovalRequest,
} from '_src/shared/messaging/messages/payloads/transactions/approvalRequest';
import { useEffect, useMemo } from 'react';
import { useParams } from 'react-router-dom';
import { Loading } from '_components';
import { useAppSelector } from '_hooks';
import { type RootState } from '../../redux/rootReducer';
import { txRequestsSelectors } from '../../redux/slices/transaction-requests';
import { SignMessageRequest } from './SignMessageRequest';
import { TransactionRequest } from './transaction-request';
import { AppType, getFromLocationSearch } from '../../redux/slices/app/appType';
import { SidePanel } from '_src/polyfills/sidepanel';

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
            if (getFromLocationSearch() == AppType.SidePanel) {
                SidePanel.enableAndGoTo(`${location.pathname}?type=sidepanel`);
            } else {
                window.close();
            }
        }
    }, [requestsLoading, request]);
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
