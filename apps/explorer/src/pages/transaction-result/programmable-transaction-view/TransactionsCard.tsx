// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaTransaction } from '@iota/iota-sdk/client';

import { Transaction } from './Transaction';
import { CollapsibleCard, ProgrammableTxnBlockCard } from '~/components';
import { TitleSize } from '@iota/apps-ui-kit';

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
            <CollapsibleCard key={index} title={type} titleSize={TitleSize.Small} collapsible>
                <div data-testid="transactions-card-content">
                    <div className="px-md pb-lg pt-xs">
                        <Transaction key={index} type={type} data={data} />
                    </div>
                </div>
            </CollapsibleCard>
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
