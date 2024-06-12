// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaTransaction } from '@iota/iota.js/client';

import { Transaction } from './Transaction';
import { ProgrammableTxnBlockCard } from '~/components/transactions/ProgTxnBlockCard';
import { CollapsibleSection } from '~/ui/Collapsible/CollapsibleSection';

interface TransactionsCardProps {
    transactions: IotaTransaction[];
}

export function TransactionsCard({ transactions }: TransactionsCardProps): JSX.Element | null {
    if (!transactions?.length) {
        return null;
    }

    const expandableItems = transactions.map((transaction, index) => {
        const [[type, data]] = Object.entries(transaction);

        return (
            <CollapsibleSection defaultOpen key={index} title={type}>
                <div data-testid="transactions-card-content">
                    <Transaction key={index} type={type} data={data} />
                </div>
            </CollapsibleSection>
        );
    });

    return (
        <ProgrammableTxnBlockCard
            initialClose
            items={expandableItems}
            itemsLabel={transactions.length > 1 ? 'Transactions' : 'Transaction'}
            count={transactions.length}
        />
    );
}
