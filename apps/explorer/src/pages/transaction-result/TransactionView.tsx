// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type Network, type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import clsx from 'clsx';
import { useState } from 'react';
import { ErrorBoundary, SplitPanes } from '~/components';
import { Events } from '~/pages/transaction-result/Events';
import { TransactionData } from '~/pages/transaction-result/TransactionData';
import { TransactionSummary } from '~/pages/transaction-result/transaction-summary';
import { Signatures } from './Signatures';
import { TransactionDetails } from './transaction-summary/TransactionDetails';
import { useTransactionSummary, useRecognizedPackages } from '@iota/core';
import { useBreakpoint, useNetwork } from '~/hooks';
import {
    ButtonSegment,
    ButtonSegmentType,
    Divider,
    Panel,
    SegmentedButton,
    SegmentedButtonType,
} from '@iota/apps-ui-kit';
import { LocalStorageSplitPaneKey } from '~/lib';

interface TransactionViewProps {
    transaction: IotaTransactionBlockResponse;
}

enum TabCategory {
    Summary = 'summary',
    Events = 'events',
    Signatures = 'signatures',
}

export function TransactionView({ transaction }: TransactionViewProps): JSX.Element {
    const [activeTab, setActiveTab] = useState<string>(TabCategory.Summary);
    const [isCollapsed, setIsCollapsed] = useState(false);

    const hasEvents = !!transaction.events?.length;
    const isMediumOrAbove = useBreakpoint('md');
    const transactionKindName = transaction.transaction?.data.transaction?.kind;
    const isProgrammableTransaction = transactionKindName === 'ProgrammableTransaction';

    const [network] = useNetwork();
    const recognizedPackagesList = useRecognizedPackages(network as Network);
    const summary = useTransactionSummary({
        transaction,
        recognizedPackagesList,
    });

    const leftPane = {
        panel: (
            <div className="flex flex-col gap-md md:flex-row">
                <div className="flex h-full w-full flex-1 overflow-auto md:w-1/3">
                    <div className="w-full">
                        <Panel>
                            <SegmentedButton type={SegmentedButtonType.Transparent}>
                                <ButtonSegment
                                    onClick={() => setActiveTab(TabCategory.Summary)}
                                    label="Summary"
                                    selected={activeTab === TabCategory.Summary}
                                    type={ButtonSegmentType.Underlined}
                                />
                                {hasEvents && (
                                    <ButtonSegment
                                        onClick={() => setActiveTab(TabCategory.Events)}
                                        label="Events"
                                        selected={activeTab === TabCategory.Events}
                                        type={ButtonSegmentType.Underlined}
                                    />
                                )}
                                {isProgrammableTransaction && (
                                    <ButtonSegment
                                        onClick={() => setActiveTab(TabCategory.Signatures)}
                                        label="Signatures"
                                        selected={activeTab === TabCategory.Signatures}
                                        type={ButtonSegmentType.Underlined}
                                    />
                                )}
                            </SegmentedButton>
                            {activeTab === TabCategory.Summary && (
                                <TransactionSummary transaction={transaction} />
                            )}
                            {hasEvents && activeTab === TabCategory.Events && (
                                <Events events={transaction.events!} />
                            )}
                            {isProgrammableTransaction && activeTab === TabCategory.Signatures && (
                                <ErrorBoundary>
                                    <Signatures transaction={transaction} />
                                </ErrorBoundary>
                            )}
                        </Panel>
                    </div>
                </div>
            </div>
        ),
        minSize: 35,
        collapsible: true,
        collapsibleButton: true,
        noHoverHidden: isMediumOrAbove,
    };
    const rightPane = {
        panel: (
            <div
                data-testid="transaction-data"
                className={clsx(
                    'h-full w-full overflow-y-auto md:overflow-y-hidden',
                    isCollapsed && isMediumOrAbove && 'pl-2',
                )}
            >
                <TransactionData transaction={transaction} />
            </div>
        ),
        minSize: 40,
        defaultSize: isProgrammableTransaction ? 65 : 50,
    };
    return (
        <div className="flex h-full flex-col gap-2xl">
            <div>
                <TransactionDetails
                    timestamp={summary?.timestamp}
                    sender={summary?.sender}
                    checkpoint={transaction.checkpoint}
                    executedEpoch={transaction.effects?.executedEpoch}
                />
            </div>
            {isMediumOrAbove ? (
                <SplitPanes
                    autoSaveId={LocalStorageSplitPaneKey.TransactionView}
                    onCollapse={setIsCollapsed}
                    splitPanels={[leftPane, rightPane]}
                    direction="horizontal"
                />
            ) : (
                <div className="flex h-full flex-col gap-lg">
                    {leftPane.panel}
                    <Divider />
                    {rightPane.panel}
                </div>
            )}
        </div>
    );
}
