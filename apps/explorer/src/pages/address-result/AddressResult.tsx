// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useParams } from 'react-router-dom';
import {
    ErrorBoundary,
    OwnedCoins,
    OwnedObjects,
    PageLayout,
    TransactionsForAddress,
} from '~/components';
import { PageHeader, SplitPanes } from '~/components/ui';
import { useBreakpoint } from '~/hooks/useBreakpoint';
import { LocalStorageSplitPaneKey } from '~/lib/enums';
import { Panel, Title, Divider } from '@iota/apps-ui-kit';
import { TotalStaked } from './TotalStaked';

const LEFT_RIGHT_PANEL_MIN_SIZE = 30;

interface AddressResultPageHeaderProps {
    address: string;
}

function AddressResultPageHeader({ address }: AddressResultPageHeaderProps): JSX.Element {
    return <PageHeader type="Address" title={address} after={<TotalStaked address={address} />} />;
}

function AddressResult({ address }: { address: string }): JSX.Element {
    return (
        <>
            <Panel>
                <Title title="Owned Objects" />
                <Divider />
                <div className="flex flex-col gap-2xl">
                    <OwnedObjectsPanel address={address} />
                </div>
            </Panel>

            <Panel>
                <Title title="Transaction Blocks" />
                <div className="flex flex-col gap-2xl p-md--rs">
                    <TransactionBlocksPanel address={address} />
                </div>
            </Panel>
        </>
    );
}

export function AddressResultPage(): JSX.Element {
    const { id } = useParams();

    return (
        <PageLayout
            content={
                <div className="flex flex-col gap-2xl">
                    <AddressResultPageHeader address={id!} />
                    <AddressResult address={id!} />
                </div>
            }
        />
    );
}

function OwnedObjectsPanel({ address }: { address: string }) {
    const isMediumOrAbove = useBreakpoint('md');
    const leftPane = {
        panel: <OwnedCoins id={address} />,
        minSize: LEFT_RIGHT_PANEL_MIN_SIZE,
        defaultSize: LEFT_RIGHT_PANEL_MIN_SIZE,
    };

    const rightPane = {
        panel: <OwnedObjects id={address} />,
        minSize: LEFT_RIGHT_PANEL_MIN_SIZE,
    };

    return (
        <div className="flex flex-col justify-between">
            <ErrorBoundary>
                {isMediumOrAbove ? (
                    <SplitPanes
                        autoSaveId={LocalStorageSplitPaneKey.AddressViewHorizontal}
                        dividerSize="none"
                        splitPanels={[leftPane, rightPane]}
                        direction="horizontal"
                    />
                ) : (
                    <>
                        {leftPane.panel}
                        <div className="my-8">
                            <Divider />
                        </div>
                        {rightPane.panel}
                    </>
                )}
            </ErrorBoundary>
        </div>
    );
}

function TransactionBlocksPanel({ address }: { address: string }) {
    return (
        <ErrorBoundary>
            <div data-testid="tx" className="relative mt-4 h-full min-h-14 overflow-auto">
                <TransactionsForAddress address={address} />
            </div>
        </ErrorBoundary>
    );
}
