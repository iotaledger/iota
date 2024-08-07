// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useActiveAddress } from '_app/hooks/useActiveAddress';
import { ErrorBoundary } from '_src/ui/app/components/error-boundary';
import ExplorerLink from '_src/ui/app/components/explorer-link';
import { ExplorerLinkType } from '_src/ui/app/components/explorer-link/ExplorerLinkType';
import { LabelValueItem } from '_src/ui/app/components/LabelValueItem';
import { LabelValuesContainer } from '_src/ui/app/components/LabelValuesContainer';
import Loading from '_src/ui/app/components/loading';
import { NFTDisplayCard } from '_src/ui/app/components/nft-display';
import { useUnlockedGuard } from '_src/ui/app/hooks/useUnlockedGuard';
import { Collapsible } from '_src/ui/app/shared/collapse';
import PageTitle from '_src/ui/app/shared/PageTitle';
import { useGetKioskContents } from '@iota/core';
import { formatAddress } from '@iota/iota.js/utils';
import { Link, useSearchParams } from 'react-router-dom';

function KioskDetailsPage() {
    const [searchParams] = useSearchParams();
    const kioskId = searchParams.get('kioskId');
    const accountAddress = useActiveAddress();
    const { data: kioskData, isPending } = useGetKioskContents(accountAddress);
    const kiosk = kioskData?.kiosks.get(kioskId!);
    const items = kiosk?.items;

    if (useUnlockedGuard()) {
        return null;
    }

    return (
        <div className="flex flex-1 flex-col flex-nowrap gap-3.75">
            <PageTitle title="Kiosk" back />
            <Loading loading={isPending}>
                {!items?.length ? (
                    <div className="text-steel-darker flex flex-1 items-center self-center text-caption font-semibold">
                        Kiosk is empty
                    </div>
                ) : (
                    <>
                        <div className="mb-auto grid grid-cols-3 items-center justify-center gap-3">
                            {items.map((item) =>
                                item.data?.objectId ? (
                                    <Link
                                        to={`/nft-details?${new URLSearchParams({
                                            objectId: item.data.objectId,
                                        }).toString()}`}
                                        key={item.data?.objectId}
                                        className="no-underline"
                                    >
                                        <ErrorBoundary>
                                            <NFTDisplayCard
                                                objectId={item.data.objectId}
                                                size="md"
                                                animateHover
                                                borderRadius="xl"
                                                isLocked={item?.isLocked}
                                            />
                                        </ErrorBoundary>
                                    </Link>
                                ) : null,
                            )}
                        </div>
                    </>
                )}
                <Collapsible defaultOpen title="Details">
                    <LabelValuesContainer>
                        <LabelValueItem label="Number of Items" value={items?.length || '0'} />
                        <LabelValueItem
                            label="Kiosk ID"
                            value={
                                <ExplorerLink
                                    className="text-hero-dark font-mono no-underline"
                                    objectID={kioskId!}
                                    type={ExplorerLinkType.Object}
                                >
                                    {formatAddress(kioskId!)}
                                </ExplorerLink>
                            }
                        />
                    </LabelValuesContainer>
                </Collapsible>
            </Loading>
        </div>
    );
}

export default KioskDetailsPage;
