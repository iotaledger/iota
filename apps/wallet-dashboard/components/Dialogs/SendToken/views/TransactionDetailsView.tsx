// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetTransaction } from '@iota/core';
import { InfoBoxType, InfoBox, InfoBoxStyle } from '@iota/apps-ui-kit';
import { Warning } from '@iota/ui-icons';
import { getExtendedTransaction } from '@/lib/utils';
import { useCurrentAccount } from '@iota/dapp-kit';
import { TransactionDetailsLayout } from '../../transaction';

interface TransactionDetailsViewProps {
    digest?: string;
    onClose: () => void;
}

export function TransactionDetailsView({ digest, onClose }: TransactionDetailsViewProps) {
    const currentAccount = useCurrentAccount();
    const { data, isError, error } = useGetTransaction(digest || '');

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

    const transaction = data && getExtendedTransaction(data, currentAccount?.address || '');

    return transaction ? (
        <TransactionDetailsLayout
            transaction={transaction}
            onClose={onClose}
            withDialogContent={false}
        />
    ) : null;
}
