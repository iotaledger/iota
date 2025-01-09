// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SignPersonalMessageApprovalRequest } from '_src/shared/messaging/messages/payloads/transactions/approvalRequest';
import { toUtf8OrB64 } from '_src/shared/utils';
import { useMemo } from 'react';
import { UserApproveContainer } from '_components';
import { useAppDispatch } from '../../hooks';
import { useAccountByAddress } from '../../hooks/useAccountByAddress';
import { useSigner } from '../../hooks/useSigner';
import { respondToTransactionRequest } from '../../redux/slices/transaction-requests';
import { PageMainLayoutTitle } from '../../shared/page-main-layout/PageMainLayoutTitle';
import { Panel } from '@iota/apps-ui-kit';

export interface SignMessageRequestProps {
    request: SignPersonalMessageApprovalRequest;
}

export function SignMessageRequest({ request }: SignMessageRequestProps) {
    const { message } = useMemo(() => toUtf8OrB64(request.tx.message), [request.tx.message]);
    const { data: account } = useAccountByAddress(request.tx.accountAddress);
    const signer = useSigner(account);
    const dispatch = useAppDispatch();

    return (
        <UserApproveContainer
            origin={request.origin}
            originFavIcon={request.originFavIcon}
            approveTitle="Sign"
            rejectTitle="Reject"
            approveDisabled={!signer}
            onSubmit={async (approved) => {
                if (!signer) {
                    return;
                }
                await dispatch(
                    respondToTransactionRequest({
                        txRequestID: request.id,
                        approved,
                        signer,
                    }),
                );
            }}
            address={request.tx.accountAddress}
            scrollable
            blended
            checkAccountLock
        >
            <PageMainLayoutTitle title="Sign Message" />
            <div className="py-md">
                <span className="text-title-lg text-neutral-10 dark:text-neutral-92">
                    Message You Are Signing
                </span>
            </div>
            <Panel hasBorder>
                <div className="flex flex-col flex-nowrap items-stretch overflow-y-auto overflow-x-hidden shadow-md">
                    <div className="break-words p-lg">
                        <span className="text-body-sm text-neutral-40 dark:text-neutral-60">
                            {message}
                        </span>
                    </div>
                </div>
            </Panel>
        </UserApproveContainer>
    );
}
