// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { Panel, Title } from '@iota/apps-ui-kit';
import { TransactionsList } from './TransactionsList';

export function TransactionsOverview() {
    return (
        <Panel>
            <Title title="Activity" />
            <div className="h-[400px] px-sm pb-md pt-sm xl:h-[500px]">
                <TransactionsList overflowClassName="overflow-y-auto" />
            </div>
        </Panel>
    );
}
