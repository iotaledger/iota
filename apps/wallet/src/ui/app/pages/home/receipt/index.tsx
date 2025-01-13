// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Loading, Overlay, ReceiptCard } from '_components';
import { useActiveAddress, useUnlockedGuard } from '_hooks';
import { useCallback, useMemo, useState } from 'react';
import { Navigate, useLocation, useNavigate, useSearchParams } from 'react-router-dom';
import { Checkmark, Warning } from '@iota/ui-icons';
import { InfoBox, InfoBoxType, InfoBoxStyle } from '@iota/apps-ui-kit';
import type { IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { useGetTransaction } from '@iota/core';

export function ReceiptPage() {
    const location = useLocation();
    const [searchParams] = useSearchParams();
    const [showModal, setShowModal] = useState(true);
    const activeAddress = useActiveAddress();

    // get tx results from url params
    const transactionId = searchParams.get('txdigest');
    const fromParam = searchParams.get('from');

    const initialDataFromState = location.state?.response as
        | IotaTransactionBlockResponse
        | undefined;

    const { data, isPending, isError, error } = useGetTransaction(transactionId ?? '', {
        retry: 8,
        initialData: initialDataFromState,
    });

    const navigate = useNavigate();
    // return to previous route or from param if available
    const closeReceipt = useCallback(() => {
        fromParam ? navigate(`/${fromParam}`) : navigate(-1);
    }, [fromParam, navigate]);

    const pageTitle = useMemo(() => {
        if (data) {
            const executionStatus = data.effects?.status.status;

            // TODO: Infer out better name:
            const transferName = 'Transaction';

            return `${executionStatus === 'success' ? transferName : 'Transaction Failed'}`;
        }

        return 'Transaction Failed';
    }, [/*activeAddress,*/ data]);

    const isGuardLoading = useUnlockedGuard();

    if (!transactionId || !activeAddress) {
        return <Navigate to="/transactions" replace={true} />;
    }

    return (
        <Loading loading={isPending || isGuardLoading}>
            <Overlay
                showModal={showModal}
                setShowModal={setShowModal}
                title={pageTitle}
                closeOverlay={closeReceipt}
                closeIcon={<Checkmark fill="currentColor" className="text-iota-light h-8 w-8" />}
            >
                {isError ? (
                    <div className="mb-2 flex h-full w-full items-center justify-center p-2">
                        <InfoBox
                            type={InfoBoxType.Error}
                            title="Something went wrong"
                            supportingText={error?.message ?? 'An error occurred'}
                            icon={<Warning />}
                            style={InfoBoxStyle.Default}
                        />
                    </div>
                ) : (
                    data && <ReceiptCard txn={data} activeAddress={activeAddress} />
                )}
            </Overlay>
        </Loading>
    );
}
