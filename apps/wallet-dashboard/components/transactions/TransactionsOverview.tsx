// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Panel, Title } from '@iota/apps-ui-kit';
import { TransactionsList } from './TransactionsList';

export function TransactionsOverview() {
    return (
        <Panel>
            <Title title="Activity" />
            <div className="h-full max-h-[400px] flex-1 overflow-y-auto px-sm pb-md  pt-sm sm:max-h-none">
                <TransactionsList heightClassName="h-full" />
            </div>
        </Panel>
    );
}
