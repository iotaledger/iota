// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
'use client';

import { VirtualList } from '@/components';
import {
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
} from '@/lib/constants/migration.constants';
import { useGetOwnedObjects } from '@iota/core';
import { useCurrentAccount, useIotaClientContext } from '@iota/dapp-kit';
import { getNetwork, IotaObjectData } from '@iota/iota-sdk/client';

function MigrationDashboardPage(): JSX.Element {
    const account = useCurrentAccount();
    const { network } = useIotaClientContext();
    const { explorer } = getNetwork(network);

    const {
        data: basicOutputObjects,
        fetchNextPage: fetchNextPageBasic,
        hasNextPage: hasNextPageBasic,
        isFetchingNextPage: isFetchingNextPageBasic,
    } = useGetOwnedObjects(account?.address || '', {
        StructType: STARDUST_BASIC_OUTPUT_TYPE,
    });
    const {
        data: nftOutputObjects,
        fetchNextPage: fetchNextPageNft,
        hasNextPage: hasNextPageNft,
        isFetchingNextPage: isFetchingNextPageNft,
    } = useGetOwnedObjects(account?.address || '', {
        StructType: STARDUST_NFT_OUTPUT_TYPE,
    });

    const basicOutputs =
        basicOutputObjects?.pages
            .flatMap((page) => page.data)
            .filter((asset) => asset.data && asset.data.objectId)
            .map((response) => response.data!) ?? [];

    const nftOutputs =
        nftOutputObjects?.pages
            .flatMap((page) => page.data)
            .filter((asset) => asset.data && asset.data.objectId)
            .map((response) => response.data!) ?? [];

    const virtualItem = (asset: IotaObjectData): JSX.Element => (
        <a href={`${explorer}/object/${asset.objectId}`} target="_blank" rel="noreferrer">
            {asset.objectId}
        </a>
    );

    return (
        <div className="flex h-full w-full flex-row items-center justify-center space-y-4">
            <div className="flex w-1/2 flex-col">
                <h1>Basic Outputs</h1>
                <VirtualList
                    items={basicOutputs}
                    hasNextPage={hasNextPageBasic}
                    isFetchingNextPage={isFetchingNextPageBasic}
                    fetchNextPage={fetchNextPageBasic}
                    estimateSize={() => 30}
                    render={virtualItem}
                />
            </div>
            <div className="flex w-1/2 flex-col">
                <h1>Nft Outputs</h1>
                <VirtualList
                    items={nftOutputs}
                    hasNextPage={hasNextPageNft}
                    isFetchingNextPage={isFetchingNextPageNft}
                    fetchNextPage={fetchNextPageNft}
                    estimateSize={() => 30}
                    render={virtualItem}
                />
            </div>
        </div>
    );
}

export default MigrationDashboardPage;
