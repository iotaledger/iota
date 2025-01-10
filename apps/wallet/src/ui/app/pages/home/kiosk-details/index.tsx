// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useActiveAddress } from '_app/hooks/useActiveAddress';
import {
    ErrorBoundary,
    ExplorerLink,
    ExplorerLinkType,
    Loading,
    NFTDisplayCard,
    PageTemplate,
} from '_components';
import { useUnlockedGuard } from '_src/ui/app/hooks/useUnlockedGuard';
import { useGetKioskContents, Collapsible } from '@iota/core';
import { formatAddress } from '@iota/iota-sdk/utils';
import { Link, useSearchParams, useNavigate } from 'react-router-dom';
import cl from 'clsx';
import { KeyValueInfo } from '@iota/apps-ui-kit';

export function KioskDetailsPage() {
    const navigate = useNavigate();
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
        <PageTemplate title="Kiosk" isTitleCentered onClose={() => navigate(-1)}>
            <div
                className={cl('flex h-full flex-1 flex-col flex-nowrap gap-5', {
                    'items-center': isPending,
                })}
            >
                <Loading loading={isPending}>
                    {!items?.length ? (
                        <div className="text-steel-darker text-caption flex flex-1 items-center self-center font-semibold">
                            Kiosk is empty
                        </div>
                    ) : (
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
                                                isHoverable
                                            />
                                        </ErrorBoundary>
                                    </Link>
                                ) : null,
                            )}
                        </div>
                    )}
                    <Collapsible defaultOpen title="Details">
                        <div className="flex flex-col gap-y-sm px-md py-xs">
                            <KeyValueInfo
                                keyText="Number of Items"
                                value={items?.length || '0'}
                                fullwidth
                            />
                            <KeyValueInfo
                                keyText="Kiosk ID"
                                value={
                                    <ExplorerLink
                                        className="text-hero-dark font-mono no-underline"
                                        objectID={kioskId!}
                                        type={ExplorerLinkType.Object}
                                    >
                                        {formatAddress(kioskId!)}
                                    </ExplorerLink>
                                }
                                fullwidth
                            />
                        </div>
                    </Collapsible>
                </Loading>
            </div>
        </PageTemplate>
    );
}
