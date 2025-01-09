// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Panel, Title, TitleSize } from '@iota/apps-ui-kit';
import { TransactionsList } from '@/components';

function ActivityPage(): JSX.Element {
    return (
        <div className="flex justify-center">
            <div className="w-3/4">
                <Panel>
                    <div className="pt-md">
                        <Title title="Activity" size={TitleSize.Medium} />
                    </div>
                    <div className="px-sm pb-md pt-sm">
                        <TransactionsList />
                    </div>
                </Panel>
            </div>
        </div>
    );
}

export default ActivityPage;
