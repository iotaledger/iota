// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { isIotaNSName, useResolveIotaNSAddress, useResolveIotaNSName } from '@iota/core';
import { Domain32 } from '@iota/icons';
import { LoadingIndicator } from '@iota/ui';
import { useParams } from 'react-router-dom';

import {
    ErrorBoundary,
    OwnedCoins,
    OwnedObjects,
    PageLayout,
    TransactionsForAddress,
} from '~/components';
import { Divider, PageHeader, SplitPanes, TabHeader, TabsList, TabsTrigger } from '~/components/ui';
import { useBreakpoint } from '~/hooks/useBreakpoint';
import { LocalStorageSplitPaneKey } from '~/lib/enums';
import { TotalStaked } from './TotalStaked';

interface AddressResultPageHeaderProps {
    address: string;
    loading?: boolean;
}

const LEFT_RIGHT_PANEL_MIN_SIZE = 30;
const TOP_PANEL_MIN_SIZE = 20;

interface AddressResultPageHeaderProps {
    address: string;
    loading?: boolean;
}
function AddressResultPageHeader({ address, loading }: AddressResultPageHeaderProps): JSX.Element {
    const { data: domainName, isLoading } = useResolveIotaNSName(address);

    return (
        <PageHeader
            loading={loading || isLoading}
            type="Address"
            title={address}
            subtitle={domainName}
            before={<Domain32 className="h-6 w-6 text-steel-darker sm:h-10 sm:w-10" />}
            after={<TotalStaked address={address} />}
        />
    );
}

function IotaNSAddressResultPageHeader({ name }: { name: string }): JSX.Element {
    const { data: address, isLoading } = useResolveIotaNSAddress(name);

    return <AddressResultPageHeader address={address ?? name} loading={isLoading} />;
}

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

function IotaNSAddressResult({ name }: { name: string }): JSX.Element {
    const { isFetched, data } = useResolveIotaNSAddress(name);

    if (!isFetched) {
        return <LoadingIndicator />;
    }

    // Fall back into just trying to load the name as an address anyway:
    return <AddressResult address={data ?? name} />;
}

export default function AddressResultPage(): JSX.Element {
    const { id } = useParams();
    const isIotaNSAddress = isIotaNSName(id!);

    return (
        <PageLayout>
            {isIotaNSAddress ? (
                <>
                    <IotaNSAddressResultPageHeader name={id!} />
                    <IotaNSAddressResult name={id!} />
                </>
            ) : (
                <>
                    <AddressResultPageHeader address={id!} />
                    <AddressResult address={id!} />
                </>
            )}
        </PageLayout>
    );
}
