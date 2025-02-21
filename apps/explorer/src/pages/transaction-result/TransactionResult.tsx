// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetTransaction } from '@iota/core';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import { useParams } from 'react-router-dom';
import { PageLayout } from '~/components';
import { PageHeader } from '~/components/ui';
import { TransactionView } from './TransactionView';
import { InfoBox, InfoBoxType, InfoBoxStyle } from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';

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
            status={txnStatus}
        />
    );
}

export function TransactionResult(): JSX.Element {
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
                <div className="flex flex-col gap-2xl">
                    <TransactionResultPageHeader
                        transaction={data}
                        error={txnErrorText}
                        loading={isPending}
                    />
                    {getTxnErrorBool || !data ? (
                        <InfoBox
                            title="Error extracting data"
                            supportingText={
                                !id
                                    ? "Can't search for a transaction without a digest"
                                    : `Data could not be extracted for the following specified transaction ID: ${id}`
                            }
                            icon={<Warning />}
                            type={InfoBoxType.Error}
                            style={InfoBoxStyle.Elevated}
                        />
                    ) : (
                        <TransactionView transaction={data} />
                    )}
                </div>
            }
        />
    );
}
