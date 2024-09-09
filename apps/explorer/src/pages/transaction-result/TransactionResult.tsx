// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetTransaction } from '@iota/core';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { useParams } from 'react-router-dom';

import { PageLayout } from '~/components';
import { Banner, PageHeader, StatusIcon } from '~/components/ui';
import { TransactionView } from './TransactionView';

interface TransactionResultPageHeaderProps {
    transaction?: IotaTransactionBlockResponse;
    error?: string;
    loading?: boolean;
}

function TransactionResultPageHeader({
    transaction,
    error,
    loading,
}: TransactionResultPageHeaderProps): JSX.Element {
    const txnKindName = transaction?.transaction?.data.transaction?.kind;
    const txnDigest = transaction?.digest ?? '';
    const txnStatus = transaction?.effects?.status.status;

    const isProgrammableTransaction = txnKindName === 'ProgrammableTransaction';

    return (
        <PageHeader
            loading={loading}
            type="Transaction"
            title={txnDigest}
            subtitle={!isProgrammableTransaction ? txnKindName : undefined}
            error={error}
            before={<StatusIcon success={txnStatus === 'success'} />}
        />
    );
}

export default function TransactionResult(): JSX.Element {
    const { id } = useParams();
    const {
        isPending,
        isError: getTxnErrorBool,
        data,
        error: getTxnError,
    } = useGetTransaction(id as string);
    const txnExecutionError = data?.effects?.status.error;

    const txnErrorText = txnExecutionError || (getTxnError as Error)?.message;

    return (
        <PageLayout
            loading={isPending}
            content={
                <>
                    <TransactionResultPageHeader
                        transaction={data}
                        error={txnErrorText}
                        loading={isPending}
                    />
                    {getTxnErrorBool || !data ? (
                        <Banner variant="error" spacing="lg" fullWidth>
                            {!id
                                ? "Can't search for a transaction without a digest"
                                : `Data could not be extracted for the following specified transaction ID: ${id}`}
                        </Banner>
                    ) : (
                        <TransactionView transaction={data} />
                    )}
                </>
            }
        />
    );
}
