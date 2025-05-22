// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Panel, Title } from '@iota/apps-ui-kit';
import { TransactionsList } from './TransactionsList';

export function TransactionsOverview() {
    return (
        <Panel>
            <Title title="Activity" />
            <div className="flex h-full w-full flex-col" data-testid="home-page-activity-section">
                <TransactionsList heightClassName="h-full" />
            </div>
        </Panel>
    );
}
