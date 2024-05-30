// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { hasDisplayData, isKioskOwnerToken, useGetOwnedObjects } from '@mysten/core';
import { useKioskClient } from '@mysten/core/src/hooks/useKioskClient';
import { type IotaObjectData } from '@mysten/iota.js/client';
import { useMemo } from 'react';

import { useBuyNLargeAsset } from '../components/buynlarge/useBuyNLargeAsset';
import { useHiddenAssets } from '../pages/home/hidden-assets/HiddenAssetsProvider';

type OwnedAssets = {
	visual: IotaObjectData[];
	other: IotaObjectData[];
	hidden: IotaObjectData[];
};

export enum AssetFilterTypes {
	visual = 'visual',
	other = 'other',
}

export function useGetNFTs(address?: string | null) {
	const kioskClient = useKioskClient();
	const { asset, objectType } = useBuyNLargeAsset();
	const {
		data,
		isPending,
		error,
		isError,
		isFetchingNextPage,
		hasNextPage,
		fetchNextPage,
		isLoading,
	} = useGetOwnedObjects(
		address,
		{
			MatchNone: objectType
				? [{ StructType: '0x2::coin::Coin' }, { StructType: objectType }]
				: [{ StructType: '0x2::coin::Coin' }],
		},
		50,
	);
	const { hiddenAssetIds } = useHiddenAssets();

	const assets = useMemo(() => {
		const ownedAssets: OwnedAssets = {
			visual: [],
			other: [],
			hidden: [],
		};

		const groupedAssets = data?.pages
			.flatMap((page) => page.data)
			.filter((asset) => !hiddenAssetIds.includes(asset.data?.objectId!))
			.reduce((acc, curr) => {
				if (hasDisplayData(curr) || isKioskOwnerToken(kioskClient.network, curr))
					acc.visual.push(curr.data as IotaObjectData);
				if (!hasDisplayData(curr)) acc.other.push(curr.data as IotaObjectData);
				if (hiddenAssetIds.includes(curr.data?.objectId!))
					acc.hidden.push(curr.data as IotaObjectData);
				return acc;
			}, ownedAssets);

		if (asset?.data) {
			groupedAssets?.visual.unshift(asset.data);
		}

		return groupedAssets;
	}, [hiddenAssetIds, data?.pages, kioskClient.network, asset]);

	return {
		data: assets,
		isLoading,
		hasNextPage,
		isFetchingNextPage,
		fetchNextPage,
		isPending: isPending,
		isError: isError,
		error,
	};
}
