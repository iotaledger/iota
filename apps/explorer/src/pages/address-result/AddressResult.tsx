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
import { Divider, SplitPanes, TabHeader, TabsList, TabsTrigger } from '~/components/ui';
import { useBreakpoint } from '~/hooks/useBreakpoint';
import { LocalStorageSplitPaneKey } from '~/lib/enums';

const LEFT_RIGHT_PANEL_MIN_SIZE = 30;
const TOP_PANEL_MIN_SIZE = 20;

function AddressResult({ address }: { address: string }): JSX.Element {
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

    const topPane = {
        panel: (
            <div className="flex h-full flex-col justify-between">
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
        ),
        minSize: TOP_PANEL_MIN_SIZE,
    };

    const bottomPane = {
        panel: (
            <div className="flex h-full flex-col pt-12">
                <TabsList>
                    <TabsTrigger value="tab">Transaction Blocks</TabsTrigger>
                </TabsList>

                <ErrorBoundary>
                    <div data-testid="tx" className="relative mt-4 h-full min-h-14 overflow-auto">
                        <TransactionsForAddress address={address} type="address" />
                    </div>
                </ErrorBoundary>

                <div className="mt-0.5">
                    <Divider />
                </div>
            </div>
        ),
    };

    return (
        <TabHeader title="Owned Objects" noGap>
            {isMediumOrAbove ? (
                <div className="h-300">
                    <SplitPanes
                        autoSaveId={LocalStorageSplitPaneKey.AddressViewVertical}
                        dividerSize="none"
                        splitPanels={[topPane, bottomPane]}
                        direction="vertical"
                    />
                </div>
            ) : (
                <>
                    {topPane.panel}
                    <div className="mt-5">
                        <Divider />
                    </div>
                    {bottomPane.panel}
                </>
            )}
        </TabHeader>
    );
}

export default function AddressResultPage(): JSX.Element {
    const { id } = useParams();

    return <PageLayout content={<AddressResult address={id!} />} />;
}
