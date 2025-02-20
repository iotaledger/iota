// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Panel, Title } from '@iota/apps-ui-kit';
import { TransactionsList } from './TransactionsList';

export function TransactionsOverview() {
    return (
        <Panel>
            <Title title="Activity" />
            <div className="px-sm pb-md pt-sm">
                <TransactionsList />
            </div>
        </Panel>
    );
}
