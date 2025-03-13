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
    // const [showTransactionDropdown, setShowTransactionDropdown] = useState(false);
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
    // TODO remove network check when querying transactions with TransactionKind filter is fixed on devnet and testnet
    /*const [network] = useNetwork();
    const isTransactionKindFilterEnabled = Network.Mainnet === network || Network.Localnet === network;
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
                <div className="absolute inset-y-0 -top-1 right-sm flex items-center gap-sm text-2xl">
                    {/* TODO re-enable this when index is stable */}
                    {/*selectedCategory === ActivityCategory.Transactions &&
                    isTransactionKindFilterEnabled ? (
                        <>
                            <div className="relative z-10">
                                <Button
                                    type={ButtonType.Ghost}
                                    onClick={() => setShowTransactionDropdown((prev) => !prev)}
                                    icon={
                                        <FilterList className="h-md w-md text-neutral-10 dark:text-neutral-92" />
                                    }
                                />
                            </div>
                            <div className="absolute bottom-0 right-0 z-10 translate-y-full">
                                <Transition
                                    show={showTransactionDropdown}
                                    enter="transition duration-300"
                                    enterFrom="opacity-0 scale-75"
                                    enterTo="opacity-100 scale-100"
                                    leave="transition duration-150"
                                    leaveFrom="opacity-100 scale-100"
                                    leaveTo="opacity-0 scale-75"
                                >
                                    <Dropdown>
                                        <ListItem
                                            hideBottomBorder
                                            onClick={() =>
                                                setShowSystemTransaction(!showSystemTransactions)
                                            }
                                        >
                                            <div className="flex flex-row gap-x-xs">
                                                <span className="w-max text-label-lg">
                                                    Show System Transactions
                                                </span>
                                                <Checkbox isChecked={showSystemTransactions} />
                                            </div>
                                        </ListItem>
                                    </Dropdown>
                                </Transition>
                            </div>
                        </>
                    ) : null*/}
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
        </Panel>
    );
}
