// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetTransaction } from '@iota/core';
import { InfoBoxType, InfoBox, InfoBoxStyle } from '@iota/apps-ui-kit';
import { Warning } from '@iota/ui-icons';
import { getExtendedTransaction } from '@/lib/utils';
import { useCurrentAccount } from '@iota/dapp-kit';
import { TransactionDialogDetails } from '../../transaction';

interface SentSuccessProps {
    digest?: string;
    onClose: () => void;
}

export function SentSuccessView({ digest, onClose }: SentSuccessProps) {
    const currentAccount = useCurrentAccount();
    const { data, isError, error } = useGetTransaction(digest || '');

    const transaction = data && getExtendedTransaction(data, currentAccount?.address || '');

    if (isError) {
        return (
            <InfoBox
                type={InfoBoxType.Error}
                title="Something went wrong"
                supportingText={error?.message ?? 'An error occurred'}
                icon={<Warning />}
                style={InfoBoxStyle.Default}
            />
        );
    }

    return transaction ? (
        <TransactionDialogDetails transaction={transaction} onClose={onClose} />
    ) : null;
}
