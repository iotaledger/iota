// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useFeatureIsOn } from '@growthbook/growthbook-react';
import { useState } from 'react';
import { Feature, toast } from '@iota/core';
import { CheckpointsTable } from '../checkpoints/CheckpointsTable';
import { EpochsActivityTable } from './EpochsActivityTable';
import { TransactionsActivityTable } from './TransactionsActivityTable';
import { PlayPause } from '~/components/ui';
import {
    ButtonSegment,
    ButtonSegmentType,
    Panel,
    SegmentedButton,
    SegmentedButtonType,
    Toggle,
    ToggleSize,
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
    const pollingTxnTableEnabled = useFeatureIsOn(Feature.PollingTxnTable as string);

    const [paused, setPaused] = useState(false);
    const [showSystemTransactions, setshowSystemTransactions] = useState(true);
    const [selectedCategory, setSelectedCategory] = useState<ActivityCategory>(
        ActivityCategory.Transactions,
    );

    const handlePauseChange = () => {
        if (paused) {
            toast(`Auto-refreshing on - every ${REFETCH_INTERVAL_SECONDS} seconds`, {
                id: AUTO_REFRESH_ID,
            });
        } else {
            toast('Auto-refresh paused', { id: AUTO_REFRESH_ID });
        }

        setPaused((paused) => !paused);
    };

    const refetchInterval = paused || !pollingTxnTableEnabled ? undefined : REFETCH_INTERVAL;
    return (
        <Panel>
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
                {pollingTxnTableEnabled && selectedCategory === ActivityCategory.Transactions && (
                    <div className="absolute inset-y-0 -top-1 right-sm flex items-center gap-sm text-2xl">
                        <Toggle
                            label="Show System Transactions"
                            isToggled={showSystemTransactions}
                            size={ToggleSize.Small}
                            onChange={() => setshowSystemTransactions(!showSystemTransactions)}
                        />
                        <PlayPause paused={paused} onChange={handlePauseChange} />
                    </div>
                )}
            </div>
            <div className="p-md">
                {selectedCategory === ActivityCategory.Transactions && (
                    <TransactionsActivityTable
                        refetchInterval={refetchInterval}
                        initialLimit={initialLimit}
                        disablePagination={disablePagination}
                        transactionKindFilter={
                            showSystemTransactions ? undefined : 'ProgrammableTransaction'
                        }
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
        </Panel>
    );
}
