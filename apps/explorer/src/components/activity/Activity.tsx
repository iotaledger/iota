// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeatureIsOn } from '@growthbook/growthbook-react';
import { useState } from 'react';
import toast from 'react-hot-toast';

import { CheckpointsTable } from '../checkpoints/CheckpointsTable';
import { EpochsActivityTable } from './EpochsActivityTable';
import { TransactionsActivityTable } from './TransactionsActivityTable';
import { PlayPause } from '~/components/ui';
import {
    ButtonSegment,
    ButtonSegmentType,
    SegmentedButton,
    SegmentedButtonType,
} from '@iota/apps-ui-kit';

enum ActivityCategory {
    Transactions = 'transactions',
    Epochs = 'epochs',
    Checkpoints = 'checkpoints',
}
const ACTIVITY_CATEGORIES = [
    {
        label: 'Transactions',
        value: ActivityCategory.Transactions,
    },
    {
        label: 'Epochs',
        value: ActivityCategory.Epochs,
    },
    {
        label: 'Checkpoints',
        value: ActivityCategory.Checkpoints,
    },
];

type ActivityProps = {
    initialTab?: string | null;
    initialLimit: number;
    disablePagination?: boolean;
};

const AUTO_REFRESH_ID = 'auto-refresh';
const REFETCH_INTERVAL_SECONDS = 10;
const REFETCH_INTERVAL = REFETCH_INTERVAL_SECONDS * 1000;

export function Activity({ initialLimit, disablePagination }: ActivityProps): JSX.Element {
    const pollingTxnTableEnabled = useFeatureIsOn('polling-txn-table');

    const [paused, setPaused] = useState(false);
    const [selectedCategory, setSelectedCategory] = useState<ActivityCategory>(
        ActivityCategory.Transactions,
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
        <>
            <div className="relative">
                <SegmentedButton type={SegmentedButtonType.Transparent}>
                    {ACTIVITY_CATEGORIES.map(({ label, value }) => (
                        <ButtonSegment
                            key={value}
                            onClick={() => setSelectedCategory(value)}
                            label={label}
                            selected={selectedCategory === value}
                            type={ButtonSegmentType.Underlined}
                        />
                    ))}
                </SegmentedButton>
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
                    {pollingTxnTableEnabled &&
                        selectedCategory === ActivityCategory.Transactions && (
                            <PlayPause paused={paused} onChange={handlePauseChange} />
                        )}
                </div>
            </div>
            <div className="p-md">
                {selectedCategory === ActivityCategory.Transactions && (
                    <TransactionsActivityTable
                        refetchInterval={refetchInterval}
                        initialLimit={initialLimit}
                        disablePagination={disablePagination}
                        transactionKindFilter={undefined}
                    />
                )}
                {selectedCategory === ActivityCategory.Epochs && (
                    <EpochsActivityTable
                        refetchInterval={refetchInterval}
                        initialLimit={initialLimit}
                        disablePagination={disablePagination}
                    />
                )}
                {selectedCategory === ActivityCategory.Checkpoints && (
                    <CheckpointsTable
                        refetchInterval={refetchInterval}
                        initialLimit={initialLimit}
                        disablePagination={disablePagination}
                    />
                )}
            </div>
        </>
    );
}
