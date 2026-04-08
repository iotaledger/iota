// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetTransaction } from '@iota/core';
import { InfoBoxType, InfoBox, InfoBoxStyle } from '@iota/apps-ui-kit';
import { Warning, Loader } from '@iota/apps-ui-icons';
import { TransactionDetailsLayout } from '../../transaction';

interface TransactionDetailsViewProps {
    digest?: string;
    onClose: () => void;
}

export function TransactionDetailsView({ digest, onClose }: TransactionDetailsViewProps) {
    const { data, isError, error, isFetching } = useGetTransaction(digest || '');

    if (isError) {
        return (
            <InfoBox
                type={InfoBoxType.Error}
                title="Error getting transaction info"
                supportingText={
                    error?.message ?? 'An error occurred when getting the transaction info'
                }
                icon={<Warning />}
                style={InfoBoxStyle.Default}
            />
        );
    }

    if (isFetching) {
        return (
            <div className="flex h-full w-full items-center justify-center">
                <Loader className="h-6 w-6 animate-spin" />
            </div>
        );
    }

    return data ? <TransactionDetailsLayout transaction={data} onClose={onClose} /> : null;
}
