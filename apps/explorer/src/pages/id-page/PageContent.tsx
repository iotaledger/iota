// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetObject } from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { useQuery } from '@tanstack/react-query';
import {
    ErrorBoundary,
    OwnedCoins,
    OwnedObjects,
    TransactionBlocksForAddress,
    TransactionsForAddressTable,
} from '~/components';
import { Banner, Divider, SplitPanes, TabHeader } from '~/components/ui';
import { useBreakpoint } from '~/hooks/useBreakpoint';
import { LocalStorageSplitPaneKey } from '~/lib/enums';
import { FieldsContent } from '~/pages/object-result/views/TokenView';

const LEFT_RIGHT_PANEL_MIN_SIZE = 30;

interface OwnedObjectsSectionProps {
    address: string;
}

function OwnedObjectsSection({ address }: OwnedObjectsSectionProps): JSX.Element {
    const isMediumOrAbove = useBreakpoint('md');

    const leftPane = {
        panel: (
            <div className="mb-5 h-full md:h-coinsAndAssetsContainer">
                <OwnedCoins id={address} />
            </div>
        ),
        minSize: LEFT_RIGHT_PANEL_MIN_SIZE,
        defaultSize: LEFT_RIGHT_PANEL_MIN_SIZE,
    };

    const rightPane = {
        panel: (
            <div className="mb-5 h-full md:h-coinsAndAssetsContainer">
                <OwnedObjects id={address} />
            </div>
        ),
        minSize: LEFT_RIGHT_PANEL_MIN_SIZE,
    };

    return (
        <TabHeader title="Owned Objects" noGap>
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
        </TabHeader>
    );
}

interface TransactionsSectionProps {
    address: string;
    isObject: boolean;
}

function TransactionsSection({ address, isObject }: TransactionsSectionProps): JSX.Element {
    const client = useIotaClient();

    const {
        data: transactionsForAddressData,
        isPending,
        isError,
    } = useQuery({
        queryKey: ['transactions-for-address', address],
        queryFn: () =>
            client.queryTransactionBlocks({
                filter: {
                    FromAndToAddress: {
                        from: address,
                        to: address,
                    },
                },
                order: 'descending',
                limit: 100,
                options: {
                    showEffects: true,
                    showInput: true,
                },
            }),
        enabled: !isObject,
    });

    return (
        <ErrorBoundary>
            {isObject ? (
                <TransactionBlocksForAddress address={address} />
            ) : (
                <TransactionsForAddressTable
                    data={transactionsForAddressData?.data ?? []}
                    isPending={isPending}
                    isError={isError}
                    address={address}
                />
            )}
        </ErrorBoundary>
    );
}

interface PageContentProps {
    address: string;
    error?: Error | null;
}

export function PageContent({ address, error }: PageContentProps): JSX.Element {
    const { data } = useGetObject(address);
    const isObject = !!data?.data;

    if (error) {
        return (
            <Banner variant="error" spacing="lg" fullWidth>
                Data could not be extracted on the following specified address ID: {address}
            </Banner>
        );
    }

    return (
        <div>
            <section>
                <OwnedObjectsSection address={address} />
            </section>

            <Divider />

            {isObject && (
                <section className="mt-14">
                    <FieldsContent objectId={address} />
                </section>
            )}

            <section className="mt-14">
                <TabHeader title="Transaction Blocks">
                    <TransactionsSection address={address} isObject={isObject} />
                </TabHeader>
            </section>
        </div>
    );
}
