// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeatureIsOn } from '@growthbook/growthbook-react';
import { Heading } from '@iota/ui';
import { useState } from 'react';
import toast from 'react-hot-toast';

import { CheckpointsTable } from '../checkpoints/CheckpointsTable';
import { EpochsActivityTable } from './EpochsActivityTable';
import { TransactionsActivityTable } from './TransactionsActivityTable';
import { PlayPause, Tabs, TabsContent, TabsList, TabsTrigger } from '~/components/ui';
import { Panel } from '@iota/apps-ui-kit';

const VALID_TABS = ['transactions', 'epochs', 'checkpoints'];

type ActivityProps = {
    initialTab?: string | null;
    initialLimit: number;
    disablePagination?: boolean;
};

const AUTO_REFRESH_ID = 'auto-refresh';
const REFETCH_INTERVAL_SECONDS = 10;
const REFETCH_INTERVAL = REFETCH_INTERVAL_SECONDS * 1000;

export function Activity({
    initialTab,
    initialLimit,
    disablePagination,
}: ActivityProps): JSX.Element {
    const pollingTxnTableEnabled = useFeatureIsOn('polling-txn-table');

    const [paused, setPaused] = useState(false);
    const [activeTab, setActiveTab] = useState(() =>
        initialTab && VALID_TABS.includes(initialTab) ? initialTab : 'transactions',
    );

    const handlePauseChange = () => {
        if (paused) {
            toast.success(`Auto-refreshing on - every ${REFETCH_INTERVAL_SECONDS} seconds`, {
                id: AUTO_REFRESH_ID,
            });
        } else {
            toast.success('Auto-refresh paused', { id: AUTO_REFRESH_ID });
        }

        setPaused((paused) => !paused);
    };

    const refetchInterval = paused || !pollingTxnTableEnabled ? undefined : REFETCH_INTERVAL;
    // TODO remove network check when querying transactions with TransactionKind filter is fixed on devnet and testnet
    /*const [network] = useNetwork();
	const isTransactionKindFilterEnabled = Network.MAINNET === network || Network.LOCAL === network;
	const [showSystemTransactions, setShowSystemTransaction] = useState(
		!isTransactionKindFilterEnabled,
	);
	useEffect(() => {
		if (!isTransactionKindFilterEnabled) {
			setShowSystemTransaction(true);
		}
	}, [isTransactionKindFilterEnabled]);*/

    return (
        <Panel>
            <Tabs size="lg" value={activeTab} onValueChange={setActiveTab}>
                <div className="relative">
                    <TabsList>
                        <TabsTrigger value="transactions">
                            <Heading variant="heading4/semibold">Transaction Blocks</Heading>
                        </TabsTrigger>
                        <TabsTrigger value="epochs">
                            <Heading variant="heading4/semibold">Epochs</Heading>
                        </TabsTrigger>
                        <TabsTrigger value="checkpoints">
                            <Heading variant="heading4/semibold">Checkpoints</Heading>
                        </TabsTrigger>
                    </TabsList>
                    <div className="absolute inset-y-0 -top-1 right-0 flex items-center gap-3 text-2xl">
                        {/* TODO re-enable this when index is stable */}
                        {/*activeTab === 'transactions' && isTransactionKindFilterEnabled ? (
							<DropdownMenu
								trigger={<Filter16 className="p-1" />}
								content={
									<DropdownMenuCheckboxItem
										checked={showSystemTransactions}
										label="Show System Transactions"
										onSelect={(e) => {
											e.preventDefault();
										}}
										onCheckedChange={() => {
											setShowSystemTransaction((value) => !value);
										}}
									/>
								}
								modal={false}
								align="end"
							/>
						) : null */}
                        {/* todo: re-enable this when rpc is stable */}
                        {pollingTxnTableEnabled && activeTab === 'transactions' && (
                            <PlayPause paused={paused} onChange={handlePauseChange} />
                        )}
                    </div>
                </div>
                <TabsContent value="transactions">
                    <TransactionsActivityTable
                        refetchInterval={refetchInterval}
                        initialLimit={initialLimit}
                        disablePagination={disablePagination}
                        transactionKindFilter={undefined}
                    />
                </TabsContent>
                <TabsContent value="epochs">
                    <EpochsActivityTable
                        refetchInterval={refetchInterval}
                        initialLimit={initialLimit}
                        disablePagination={disablePagination}
                    />
                </TabsContent>
                <TabsContent value="checkpoints">
                    <CheckpointsTable
                        refetchInterval={refetchInterval}
                        initialLimit={initialLimit}
                        disablePagination={disablePagination}
                    />
                </TabsContent>
            </Tabs>
        </Panel>
    );
}
