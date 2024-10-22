// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SignMessageApprovalRequest } from '_payloads/transactions/ApprovalRequest';
import { toUtf8OrB64 } from '_src/shared/utils';
import { useMemo } from 'react';
import { UserApproveContainer } from '_components';
import { useAppDispatch } from '../../hooks';
import { useAccountByAddress } from '../../hooks/useAccountByAddress';
import { useSigner } from '../../hooks/useSigner';
import { respondToTransactionRequest } from '../../redux/slices/transaction-requests';
import { PageMainLayoutTitle } from '../../shared/page-main-layout/PageMainLayoutTitle';
import { Text } from '../../shared/text';

export interface SignMessageRequestProps {
    request: SignMessageApprovalRequest;
}

export function SignMessageRequest({ request }: SignMessageRequestProps) {
    const { message, type } = useMemo(() => toUtf8OrB64(request.tx.message), [request.tx.message]);
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
            <div className="py-4">
                <span className="text-title-lg">Message You Are Signing</span>
            </div>
            <div className="flex flex-col flex-nowrap items-stretch overflow-y-auto overflow-x-hidden rounded-15 border border-solid border-gray-50 bg-white shadow-card-soft">
                <div className="break-words p-5">
                    <Text
                        variant="pBodySmall"
                        weight="medium"
                        color="steel-darker"
                        mono={type === 'base64'}
                    >
                        {message}
                    </Text>
                </div>
            </div>
        </UserApproveContainer>
    );
}
