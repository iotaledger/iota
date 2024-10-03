// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';
import clsx from 'clsx';
import { useState } from 'react';
import { ErrorBoundary } from '~/components';
import { Events } from '~/pages/transaction-result/Events';
import { TransactionData } from '~/pages/transaction-result/TransactionData';
import { TransactionSummary } from '~/pages/transaction-result/transaction-summary';
import { Signatures } from './Signatures';
import styles from './TransactionResult.module.css';
import { TransactionDetails } from './transaction-summary/TransactionDetails';
import { useTransactionSummary } from '@iota/core';
import { useRecognizedPackages } from '~/hooks';
import {
    ButtonSegment,
    ButtonSegmentType,
    Panel,
    SegmentedButton,
    SegmentedButtonType,
} from '@iota/apps-ui-kit';

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
    const hasEvents = !!transaction.events?.length;

    const transactionKindName = transaction.transaction?.data.transaction?.kind;

    const isProgrammableTransaction = transactionKindName === 'ProgrammableTransaction';

    const recognizedPackagesList = useRecognizedPackages();
    const summary = useTransactionSummary({
        transaction,
        recognizedPackagesList,
    });
    return (
        <div className={clsx(styles.txdetailsbg)}>
            <div className="flex h-full flex-col gap-2xl">
                <div>
                    <TransactionDetails
                        timestamp={summary?.timestamp}
                        sender={summary?.sender}
                        checkpoint={transaction.checkpoint}
                        executedEpoch={transaction.effects?.executedEpoch}
                    />
                </div>
                <div className="flex flex-col gap-md md:flex-row">
                    <div className="flex h-full w-full flex-1 overflow-auto md:h-full md:max-h-screen md:w-1/3">
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
                                {isProgrammableTransaction &&
                                    activeTab === TabCategory.Signatures && (
                                        <ErrorBoundary>
                                            <Signatures transaction={transaction} />
                                        </ErrorBoundary>
                                    )}
                            </Panel>
                        </div>
                    </div>
                    <div className="h-full w-full overflow-y-auto md:w-2/3 md:overflow-y-hidden">
                        <TransactionData transaction={transaction} />
                    </div>
                </div>
            </div>
        </div>
    );
}
